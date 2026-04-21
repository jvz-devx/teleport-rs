// TypeScript client function generation.
//
// Generates `client.ts` with tree-shakeable individual function exports
// and an ergonomic `bindClient(client)` wrapper.
// Imports from `types.ts` and `errors.ts`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use specta::ResolvedTypes;

use crate::config::Config;
use crate::naming::split_namespace;
use crate::{GenerateError, ProcedureInfo};

use crate::GENERATED_HEADER;

/// Generate the contents of `client.ts`.
///
/// Groups procedures by namespace and generates typed RPC call functions.
#[allow(clippy::too_many_lines)]
pub(crate) fn generate_client(
    procedures: &[ProcedureInfo],
    config: &Config,
    resolved_types: &ResolvedTypes,
) -> Result<String, GenerateError> {
    let ts_exporter = specta_typescript::Typescript::default();

    // Resolve all TS type strings and collect import names.
    let mut type_imports: BTreeSet<String> = BTreeSet::new();

    // Group procedures by namespace.
    let mut namespaces: BTreeMap<String, Vec<ProcedureEntry>> = BTreeMap::new();

    for proc in procedures {
        let input_ts =
            crate::ts_utils::datatype_to_ts(&ts_exporter, resolved_types, &proc.input_type)?;
        let output_ts =
            crate::ts_utils::datatype_to_ts(&ts_exporter, resolved_types, &proc.output_type)?;
        let error_ts =
            crate::ts_utils::datatype_to_ts(&ts_exporter, resolved_types, &proc.error_type)?;

        // Collect non-primitive type names for imports.
        collect_type_names(&proc.input_type, resolved_types, &mut type_imports);
        collect_type_names(&proc.output_type, resolved_types, &mut type_imports);
        collect_type_names(&proc.error_type, resolved_types, &mut type_imports);

        let (ns, method_name) = split_namespace(&proc.name);
        let namespace = if ns.is_empty() {
            "_root".to_owned()
        } else {
            ns.to_owned()
        };

        let output_ts_return = if is_unit_type(&output_ts) {
            "void".to_owned()
        } else {
            output_ts
        };

        // Procedures with no typed error detail have `AppError<()>`, which
        // `datatype_to_ts` renders as `null`. That's technically a value
        // (nullable) — semantically wrong for "there is no detail type".
        // Rewrite to `never` so the TS type `Promise<RpcResult<T, never>>`
        // correctly expresses "the error variant carries no detail".
        let error_ts = if is_unit_type(&error_ts) {
            "never".to_owned()
        } else {
            error_ts
        };

        let is_void_input = is_unit_type(&input_ts);

        namespaces
            .entry(namespace)
            .or_default()
            .push(ProcedureEntry {
                method_name: method_name.to_owned(),
                http_method: proc.method.as_str().to_owned(),
                path: proc.path.clone(),
                doc: proc.doc.clone(),
                input_ts: if is_void_input { None } else { Some(input_ts) },
                output_ts: output_ts_return,
                error_ts,
            });
    }

    // Remove primitive type names from imports (they don't need importing).
    // `is_ts_stdlib_wrapper` is a defence-in-depth filter: even though
    // `datatype_to_ts` translates `Vec<T>` / `String` / `HashMap<K,V>` /
    // etc. into inline TS constructs, these names may still be collected
    // by `collect_type_names` below because specta registers them as
    // `NamedDataType`s. Scrub them here so they never appear in the
    // `import type { … } from "./types"` line.
    type_imports.retain(|name| {
        !crate::ts_utils::is_ts_primitive(name) && !crate::ts_utils::is_ts_stdlib_wrapper(name)
    });

    // Build output.
    let mut out = String::with_capacity(2048);
    out.push_str(GENERATED_HEADER);
    out.push('\n');
    out.push('\n');

    let client_path = config
        .client_import_path
        .as_deref()
        .unwrap_or("@teleport-rs/client");

    // Import RpcResult and TeleportClient from the client runtime package.
    let _ = writeln!(
        out,
        "import type {{ RpcResult, TeleportClient }} from \"{client_path}\";",
    );

    // Import data types from types.ts.
    if !type_imports.is_empty() {
        let imports: Vec<&str> = type_imports.iter().map(String::as_str).collect();
        let _ = writeln!(
            out,
            "import type {{ {} }} from \"./types\";",
            imports.join(", ")
        );
    }

    // Generate tree-shakeable individual function exports.
    for (ns_name, entries) in &namespaces {
        for entry in entries {
            out.push('\n');

            // JSDoc comment.
            if entry.doc.is_empty() {
                let _ = writeln!(out, "/** {} {} */", entry.http_method, entry.path);
            } else {
                let _ = writeln!(out, "/**");
                for line in entry.doc.lines() {
                    let _ = writeln!(out, " * {line}");
                }
                let _ = writeln!(out, " * {} {}", entry.http_method, entry.path);
                let _ = writeln!(out, " */");
            }

            let param = entry
                .input_ts
                .as_ref()
                .map_or_else(
                    || "client: Pick<TeleportClient, \"rpc\">".to_owned(),
                    |ts| format!("client: Pick<TeleportClient, \"rpc\">, input: {ts}"),
                );

            let rpc_arg = if entry.input_ts.is_some() {
                "input"
            } else {
                "undefined"
            };

            let _ = writeln!(
                out,
                "export function {ns}_{name}({param}): Promise<RpcResult<{output}, {error}>> {{\n  return client.rpc(\"{method}\", \"{path}\", {rpc_arg});\n}}",
                ns = ns_name,
                name = entry.method_name,
                output = entry.output_ts,
                error = entry.error_ts,
                method = entry.http_method,
                path = entry.path,
            );
        }
    }

    out.push('\n');
    out.push_str(
        "export function bindClient(client: Pick<TeleportClient, \"rpc\">) {\n  return {\n",
    );

    for (ns_name, entries) in &namespaces {
        let _ = writeln!(out, "    {ns_name}: {{");

        for (i, entry) in entries.iter().enumerate() {
            let param = entry
                .input_ts
                .as_ref()
                .map_or_else(String::new, |ts| format!("input: {ts}"));
            let rpc_arg = if entry.input_ts.is_some() {
                "input"
            } else {
                "undefined"
            };

            let _ = write!(
                out,
                "      {name}({param}) {{ return client.rpc<{output}, {error}>(\"{method}\", \"{path}\", {rpc_arg}); }}",
                name = entry.method_name,
                output = entry.output_ts,
                error = entry.error_ts,
                method = entry.http_method,
                path = entry.path,
            );

            if i < entries.len() - 1 {
                out.push_str(",\n");
            } else {
                out.push('\n');
            }
        }

        out.push_str("    },\n");
    }

    out.push_str("  };\n}\n");

    Ok(out)
}

struct ProcedureEntry {
    method_name: String,
    http_method: String,
    path: String,
    doc: String,
    input_ts: Option<String>,
    output_ts: String,
    error_ts: String,
}

/// Recursively collect named type references from a `DataType`.
fn collect_type_names(
    dt: &specta::datatype::DataType,
    resolved_types: &ResolvedTypes,
    names: &mut BTreeSet<String>,
) {
    use specta::datatype::{DataType, Reference};

    match dt {
        DataType::Reference(Reference::Named(named_ref)) => {
            if let Some(ndt) = named_ref.get(resolved_types.as_types()) {
                names.insert(ndt.name().to_string());
            }
            // Also collect from generic arguments.
            for (_generic, arg_dt) in named_ref.generics() {
                collect_type_names(arg_dt, resolved_types, names);
            }
        }
        DataType::Nullable(inner) => {
            collect_type_names(inner, resolved_types, names);
        }
        DataType::List(list) => {
            collect_type_names(list.ty(), resolved_types, names);
        }
        DataType::Map(map) => {
            collect_type_names(map.key_ty(), resolved_types, names);
            collect_type_names(map.value_ty(), resolved_types, names);
        }
        DataType::Tuple(tuple) => {
            for element in tuple.elements() {
                collect_type_names(element, resolved_types, names);
            }
        }
        DataType::Struct(s) => {
            use specta::datatype::Fields;
            match s.fields() {
                Fields::Named(fields) => {
                    for (_name, field) in fields.fields() {
                        if let Some(ty) = field.ty() {
                            collect_type_names(ty, resolved_types, names);
                        }
                    }
                }
                Fields::Unnamed(fields) => {
                    for field in fields.fields() {
                        if let Some(ty) = field.ty() {
                            collect_type_names(ty, resolved_types, names);
                        }
                    }
                }
                Fields::Unit => {}
            }
        }
        DataType::Enum(e) => {
            for (_name, variant) in e.variants() {
                use specta::datatype::Fields;
                match variant.fields() {
                    Fields::Named(fields) => {
                        for (_name, field) in fields.fields() {
                            if let Some(ty) = field.ty() {
                                collect_type_names(ty, resolved_types, names);
                            }
                        }
                    }
                    Fields::Unnamed(fields) => {
                        for field in fields.fields() {
                            if let Some(ty) = field.ty() {
                                collect_type_names(ty, resolved_types, names);
                            }
                        }
                    }
                    Fields::Unit => {}
                }
            }
        }
        DataType::Primitive(_) | DataType::Reference(_) => {}
    }
}

/// Returns `true` for TS representations of the unit type.
fn is_unit_type(ts: &str) -> bool {
    ts == "null" || ts == "[]"
}

#[cfg(test)]
mod tests {
    // Test-only: `.expect()` is informative and any panic is caught by the test runner.
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::config::{Config, NamespaceStyle, Naming};
    use specta::datatype::{DataType, Primitive, Tuple};
    use specta::{Type, Types};
    use std::path::PathBuf;

    #[derive(Debug, Clone, specta::Type)]
    #[allow(dead_code)]
    struct TestUser {
        id: String,
        name: String,
    }

    #[derive(Debug, Clone, specta::Type)]
    #[allow(dead_code)]
    struct GetUserInput {
        id: String,
    }

    /// Helper to get the `DataType` for the TS `never` type.
    fn never_datatype() -> DataType {
        <specta_typescript::Never as Type>::definition(&mut Types::default())
    }

    fn test_config() -> Config {
        Config {
            output_dir: PathBuf::from("/tmp/test"),
            namespace_style: NamespaceStyle::ModulePath,
            naming: Naming::default(),

            route_prefix: "/rpc".to_owned(),
            client_import_path: None,
        }
    }

    #[test]
    fn generates_namespace_with_procedure() {
        let mut types = Types::default();
        let input_dt = <GetUserInput as Type>::definition(&mut types);
        let output_dt = <TestUser as Type>::definition(&mut types);
        let resolved = ResolvedTypes::from_resolved_types(types);

        let proc = ProcedureInfo {
            name: "users.getUser".to_owned(),
            method: crate::HttpMethod::Get,
            path: "/rpc/users.getUser".to_owned(),
            doc: "Get a user by ID".to_owned(),
            input_type: input_dt,
            output_type: output_dt,
            error_type: never_datatype(),
        };

        let config = test_config();
        let output = generate_client(&[proc], &config, &resolved).expect("should generate");

        // Tree-shakeable function export.
        assert!(output.contains("export function users_getUser("));
        assert!(output.contains("client.rpc(\"GET\""));
        assert!(output.contains("/rpc/users.getUser"));
        assert!(output.contains("export function bindClient("));
        assert!(output.contains("client.rpc<TestUser, never>(\"GET\", \"/rpc/users.getUser\", input)"));
        assert!(output.contains("@teleport-rs/client"));
    }

    #[test]
    fn void_input_passes_undefined() {
        let types = Types::default();
        let resolved = ResolvedTypes::from_resolved_types(types);

        let proc = ProcedureInfo {
            name: "health.check".to_owned(),
            method: crate::HttpMethod::Get,
            path: "/rpc/health.check".to_owned(),
            doc: String::new(),
            input_type: DataType::Tuple(Tuple::new(vec![])),
            output_type: DataType::Primitive(Primitive::str),
            error_type: never_datatype(),
        };

        let config = test_config();
        let output = generate_client(&[proc], &config, &resolved).expect("should generate");

        assert!(output.contains("undefined"));
        assert!(output.contains("export function health_check(client: Pick<TeleportClient, \"rpc\">):"));
        assert!(
            output.contains(
                "check() { return client.rpc<string, never>(\"GET\", \"/rpc/health.check\", undefined); }"
            )
        );
    }
}
