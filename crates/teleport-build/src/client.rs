use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use teleport_contract::{ContractBundle, TypeExpr};

use crate::Config;
use crate::GENERATED_HEADER;
use crate::GenerateError;

/// Generate the contents of `client.ts`.
#[allow(clippy::too_many_lines)]
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn generate_client(
    bundle: &ContractBundle,
    config: &Config,
) -> Result<String, GenerateError> {
    let mut type_imports: BTreeSet<String> = BTreeSet::new();
    let mut namespaces: BTreeMap<String, Vec<ProcedureEntry>> = BTreeMap::new();

    for proc in &bundle.procedures {
        crate::ts_utils::collect_type_names(&proc.input_type, &mut type_imports);
        crate::ts_utils::collect_type_names(&proc.output_type, &mut type_imports);
        crate::ts_utils::collect_type_names(&proc.error_type, &mut type_imports);

        let output_ts = crate::ts_utils::type_expr_to_ts(&proc.output_type);
        let output_ts_return = if is_unit_type(&proc.output_type) {
            "void".to_owned()
        } else {
            output_ts
        };

        let error_ts = if is_unit_type(&proc.error_type) {
            "never".to_owned()
        } else {
            crate::ts_utils::type_expr_to_ts(&proc.error_type)
        };

        namespaces
            .entry(proc.namespace.clone())
            .or_default()
            .push(ProcedureEntry {
                method_name: proc.method_name.clone(),
                http_method: proc.http_method.as_str().to_owned(),
                path: proc.path.clone(),
                doc: proc.doc.clone(),
                input_ts: if is_unit_type(&proc.input_type) {
                    None
                } else {
                    Some(crate::ts_utils::type_expr_to_ts(&proc.input_type))
                },
                output_ts: output_ts_return,
                error_ts,
            });
    }

    let mut out = String::with_capacity(2048);
    out.push_str(GENERATED_HEADER);
    out.push('\n');
    out.push('\n');

    let client_path = config
        .client_import_path
        .as_deref()
        .unwrap_or("@teleport-rs/client");

    let _ = writeln!(
        out,
        "import type {{ RpcResult, TeleportClient }} from \"{client_path}\";",
    );

    if !type_imports.is_empty() {
        let imports: Vec<&str> = type_imports.iter().map(String::as_str).collect();
        let _ = writeln!(
            out,
            "import type {{ {} }} from \"./types\";",
            imports.join(", ")
        );
    }

    for (ns_name, entries) in &namespaces {
        for entry in entries {
            out.push('\n');
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
                .map_or_else(String::new, |ts| format!("input: {ts}"));
            let rpc_arg = if entry.input_ts.is_some() {
                "input"
            } else {
                "undefined"
            };

            let _ = writeln!(
                out,
                "export function {ns}_{name}(client: Pick<TeleportClient, \"rpc\">{param_prefix}{param}): Promise<RpcResult<{output}, {error}>> {{\n  return client.rpc(\"{method}\", \"{path}\", {rpc_arg});\n}}",
                ns = ns_name,
                name = entry.method_name,
                param_prefix = if entry.input_ts.is_some() { ", " } else { "" },
                output = entry.output_ts,
                error = entry.error_ts,
                method = entry.http_method,
                path = entry.path,
            );
        }
    }

    for (ns_name, entries) in &namespaces {
        out.push('\n');
        let _ = writeln!(out, "export const {ns_name} = {{");
        for (i, entry) in entries.iter().enumerate() {
            let _ = write!(
                out,
                "  {name}: {ns}_{name}",
                name = entry.method_name,
                ns = ns_name,
            );
            if i < entries.len() - 1 {
                out.push_str(",\n");
            } else {
                out.push('\n');
            }
        }
        out.push_str("};\n");
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

const fn is_unit_type(expr: &TypeExpr) -> bool {
    matches!(expr, TypeExpr::Tuple(elements) if elements.is_empty())
}
