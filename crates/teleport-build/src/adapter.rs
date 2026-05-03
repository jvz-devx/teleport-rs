use std::collections::HashMap;

use specta::ResolvedTypes;
use specta::datatype::{DataType, Fields, GenericReference, Primitive, Reference};
use teleport_contract::{
    AuthMode, ContractBundle, FieldsContract, HttpMethod, InputEncoding, NamedFieldContract,
    NamedTypeContract, NamedTypeKind, PrimitiveType, ProcedureContract, ProcedureKind, TypeExpr,
    UnnamedFieldContract, VariantContract,
};

use crate::{Config, GenerateError, naming};

#[allow(clippy::print_stderr)]
#[allow(clippy::redundant_pub_crate)]
pub(super) fn contract_from_inventory(config: &Config) -> Result<ContractBundle, GenerateError> {
    let mut types = specta::Types::default();
    let mut procedures = Vec::new();

    for reg in inventory::iter::<teleport_core::ProcedureRegistration> {
        let name = reg.name();
        let namespace = reg.namespace().to_owned();
        let method_name = reg.fn_name.to_owned();
        let path = format!("{}/{}", config.route_prefix, name);
        let input_type = (reg.input_type)(&mut types);
        let output_type = (reg.output_type)(&mut types);
        let error_type = (reg.error_type)(&mut types);

        procedures.push(RawProcedure {
            name,
            namespace,
            method_name,
            procedure_kind: match reg.procedure_type {
                teleport_core::ProcedureType::Query => ProcedureKind::Query,
                teleport_core::ProcedureType::Command => ProcedureKind::Command,
                teleport_core::ProcedureType::Form => ProcedureKind::Form,
            },
            http_method: match reg.method {
                teleport_core::HttpMethod::Get => HttpMethod::Get,
                teleport_core::HttpMethod::Post => HttpMethod::Post,
            },
            path,
            input_encoding: match reg.procedure_type {
                teleport_core::ProcedureType::Query => {
                    if matches!(input_type, DataType::Tuple(ref tuple) if tuple.elements().is_empty()) {
                        InputEncoding::None
                    } else {
                        InputEncoding::QueryString
                    }
                }
                teleport_core::ProcedureType::Command => {
                    if matches!(input_type, DataType::Tuple(ref tuple) if tuple.elements().is_empty()) {
                        InputEncoding::None
                    } else {
                        InputEncoding::JsonBody
                    }
                }
                teleport_core::ProcedureType::Form => {
                    if matches!(input_type, DataType::Tuple(ref tuple) if tuple.elements().is_empty()) {
                        InputEncoding::None
                    } else {
                        InputEncoding::FormBody
                    }
                }
            },
            auth_mode: reg.auth_mode,
            doc: reg.doc.to_owned(),
            input_type,
            output_type,
            error_type,
        });
    }

    if procedures.is_empty() {
        eprintln!(
            "teleport-rs warning: no procedures found. Did you import all modules containing #[remote] functions?"
        );
    }

    procedures.sort_by(|a, b| a.name.cmp(&b.name));

    let resolved = ResolvedTypes::from_resolved_types(types);
    let named_types = collect_named_types(&resolved)?;
    let procedures = procedures
        .into_iter()
        .map(|proc| adapt_procedure(proc, &resolved))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ContractBundle {
        version: teleport_contract::CONTRACT_VERSION.to_owned(),
        procedures,
        types: named_types,
    })
}

#[derive(Debug)]
struct RawProcedure {
    name: String,
    namespace: String,
    method_name: String,
    procedure_kind: ProcedureKind,
    http_method: HttpMethod,
    path: String,
    input_encoding: InputEncoding,
    auth_mode: AuthMode,
    doc: String,
    input_type: DataType,
    output_type: DataType,
    error_type: DataType,
}

fn adapt_procedure(
    proc: RawProcedure,
    resolved: &ResolvedTypes,
) -> Result<ProcedureContract, GenerateError> {
    Ok(ProcedureContract {
        name: proc.name,
        namespace: proc.namespace,
        method_name: proc.method_name,
        procedure_kind: proc.procedure_kind,
        http_method: proc.http_method,
        path: proc.path,
        input_encoding: proc.input_encoding,
        auth_mode: proc.auth_mode,
        doc: proc.doc,
        input_type: adapt_type_expr(&proc.input_type, resolved, &HashMap::new())?,
        output_type: adapt_type_expr(&proc.output_type, resolved, &HashMap::new())?,
        error_type: adapt_type_expr(&proc.error_type, resolved, &HashMap::new())?,
    })
}

fn collect_named_types(resolved: &ResolvedTypes) -> Result<Vec<NamedTypeContract>, GenerateError> {
    let mut types = Vec::new();

    for ndt in resolved.as_types().into_sorted_iter() {
        if !ndt.requires_reference(resolved.as_types()) || is_stdlib_wrapper(ndt.name().as_ref()) {
            continue;
        }

        let generic_names = ndt
            .generics()
            .iter()
            .map(|(_, name)| name.to_string())
            .collect::<Vec<_>>();
        let generic_ctx = ndt
            .generics()
            .iter()
            .map(|(g, name)| (g.clone(), name.to_string()))
            .collect::<HashMap<_, _>>();

        let kind = match ndt.ty() {
            DataType::Struct(struct_dt) => {
                NamedTypeKind::Struct(adapt_fields(struct_dt.fields(), resolved, &generic_ctx)?)
            }
            DataType::Enum(enum_dt) => {
                let variants = enum_dt
                    .variants()
                    .iter()
                    .map(|(name, variant)| {
                        Ok(VariantContract {
                            name: name.to_string(),
                            docs: variant.docs().to_string(),
                            fields: adapt_fields(variant.fields(), resolved, &generic_ctx)?,
                        })
                    })
                    .collect::<Result<Vec<_>, GenerateError>>()?;
                NamedTypeKind::Enum(variants)
            }
            other => NamedTypeKind::Alias(adapt_type_expr(other, resolved, &generic_ctx)?),
        };

        types.push(NamedTypeContract {
            name: ndt.name().to_string(),
            docs: ndt.docs().to_string(),
            generics: generic_names,
            kind,
        });
    }

    Ok(types)
}

fn adapt_fields(
    fields: &Fields,
    resolved: &ResolvedTypes,
    generic_ctx: &HashMap<GenericReference, String>,
) -> Result<FieldsContract, GenerateError> {
    match fields {
        Fields::Unit => Ok(FieldsContract::Unit),
        Fields::Named(named) => Ok(FieldsContract::Named(
            named
                .fields()
                .iter()
                .map(|(name, field)| {
                    Ok(NamedFieldContract {
                        name: name.to_string(),
                        docs: field.docs().to_string(),
                        optional: field.optional(),
                        ty: field
                            .ty()
                            .map(|ty| adapt_type_expr(ty, resolved, generic_ctx))
                            .transpose()?,
                    })
                })
                .collect::<Result<Vec<_>, GenerateError>>()?,
        )),
        Fields::Unnamed(unnamed) => Ok(FieldsContract::Unnamed(
            unnamed
                .fields()
                .iter()
                .map(|field| {
                    Ok(UnnamedFieldContract {
                        docs: field.docs().to_string(),
                        ty: field
                            .ty()
                            .map(|ty| adapt_type_expr(ty, resolved, generic_ctx))
                            .transpose()?,
                    })
                })
                .collect::<Result<Vec<_>, GenerateError>>()?,
        )),
    }
}

fn adapt_type_expr(
    dt: &DataType,
    resolved: &ResolvedTypes,
    generic_ctx: &HashMap<GenericReference, String>,
) -> Result<TypeExpr, GenerateError> {
    Ok(match dt {
        DataType::Primitive(primitive) => TypeExpr::Primitive(adapt_primitive(primitive)),
        DataType::List(list) => {
            TypeExpr::List(Box::new(adapt_type_expr(list.ty(), resolved, generic_ctx)?))
        }
        DataType::Map(map) => TypeExpr::Map {
            key: Box::new(adapt_type_expr(map.key_ty(), resolved, generic_ctx)?),
            value: Box::new(adapt_type_expr(map.value_ty(), resolved, generic_ctx)?),
        },
        DataType::Tuple(tuple) => TypeExpr::Tuple(
            tuple
                .elements()
                .iter()
                .map(|el| adapt_type_expr(el, resolved, generic_ctx))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        DataType::Nullable(inner) => {
            TypeExpr::Nullable(Box::new(adapt_type_expr(inner, resolved, generic_ctx)?))
        }
        DataType::Struct(struct_dt) => TypeExpr::Opaque(format!(
            "inline_struct_{}",
            render_inline_fields(&adapt_fields(struct_dt.fields(), resolved, generic_ctx)?)
        )),
        DataType::Enum(enum_dt) => {
            TypeExpr::Opaque(format!("inline_enum_{}", enum_dt.variants().len()))
        }
        DataType::Reference(Reference::Named(named_ref)) => {
            if let Some(ndt) = named_ref.get(resolved.as_types()) {
                match ndt.name().as_ref() {
                    "Vec" | "VecDeque" | "HashSet" | "BTreeSet" | "LinkedList" | "BinaryHeap" => {
                        let inner = named_ref
                            .generics()
                            .first()
                            .map(|(_, dt)| adapt_type_expr(dt, resolved, generic_ctx))
                            .transpose()?
                            .unwrap_or(TypeExpr::Primitive(PrimitiveType::str));
                        TypeExpr::List(Box::new(inner))
                    }
                    "String" | "str" => TypeExpr::Primitive(PrimitiveType::str),
                    "HashMap" | "BTreeMap" => TypeExpr::Map {
                        key: Box::new(adapt_type_expr(
                            &named_ref.generics()[0].1,
                            resolved,
                            generic_ctx,
                        )?),
                        value: Box::new(adapt_type_expr(
                            &named_ref.generics()[1].1,
                            resolved,
                            generic_ctx,
                        )?),
                    },
                    _ => TypeExpr::Named {
                        name: ndt.name().to_string(),
                        generics: named_ref
                            .generics()
                            .iter()
                            .map(|(_, dt)| adapt_type_expr(dt, resolved, generic_ctx))
                            .collect::<Result<Vec<_>, _>>()?,
                    },
                }
            } else {
                return Err(GenerateError::TypeExport(
                    "named reference could not be resolved".to_owned(),
                ));
            }
        }
        DataType::Reference(Reference::Generic(generic)) => TypeExpr::Generic(
            generic_ctx
                .get(generic)
                .cloned()
                .unwrap_or_else(|| "T".to_owned()),
        ),
        DataType::Reference(Reference::Opaque(opaque)) => {
            TypeExpr::Opaque(opaque.type_name().to_owned())
        }
    })
}

fn render_inline_fields(fields: &FieldsContract) -> String {
    match fields {
        FieldsContract::Unit => "unit".to_owned(),
        FieldsContract::Named(fields) => fields
            .iter()
            .map(|field| field.name.clone())
            .collect::<Vec<_>>()
            .join("_"),
        FieldsContract::Unnamed(fields) => format!("tuple{}", fields.len()),
    }
}

const fn adapt_primitive(primitive: &Primitive) -> PrimitiveType {
    match primitive {
        Primitive::i8 => PrimitiveType::i8,
        Primitive::i16 => PrimitiveType::i16,
        Primitive::i32 => PrimitiveType::i32,
        Primitive::i64 => PrimitiveType::i64,
        Primitive::i128 => PrimitiveType::i128,
        Primitive::isize => PrimitiveType::isize,
        Primitive::u8 => PrimitiveType::u8,
        Primitive::u16 => PrimitiveType::u16,
        Primitive::u32 => PrimitiveType::u32,
        Primitive::u64 => PrimitiveType::u64,
        Primitive::u128 => PrimitiveType::u128,
        Primitive::usize => PrimitiveType::usize,
        Primitive::f16 => PrimitiveType::f16,
        Primitive::f32 => PrimitiveType::f32,
        Primitive::f64 => PrimitiveType::f64,
        Primitive::f128 => PrimitiveType::f128,
        Primitive::bool => PrimitiveType::bool,
        Primitive::char => PrimitiveType::char,
        Primitive::str => PrimitiveType::str,
    }
}

fn is_stdlib_wrapper(name: &str) -> bool {
    naming::split_namespace(name).0.is_empty() && crate::ts_utils::is_ts_stdlib_wrapper(name)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, dead_code)]

    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};
    use specta::Type;
    use teleport_core::{AuthMode as CoreAuthMode, HttpMethod as CoreHttpMethod};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, Type)]
    struct SearchInput {
        query: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Type)]
    struct CreateInput {
        name: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Type)]
    struct FormInput {
        token: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Type)]
    struct GenericEnvelope<T> {
        payload: T,
        maybe_payload: Option<T>,
        tags: Vec<String>,
        counts: HashMap<String, i32>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Type)]
    enum ProcedureError<T> {
        Idle,
        RetryAfter(u32),
        Invalid { message: String, context: Option<T> },
    }

    fn stub_mount() -> Box<dyn std::any::Any + Send> {
        Box::new(())
    }

    inventory::submit! {
        teleport_core::ProcedureRegistration {
            module_path: "adapter_tests::search",
            fn_name: "searchThings",
            prefix: None,
            method: CoreHttpMethod::Get,
            procedure_type: teleport_core::ProcedureType::Query,
            input_type: |types| <SearchInput as Type>::definition(types),
            output_type: |types| <GenericEnvelope<String> as Type>::definition(types),
            error_type: |types| <ProcedureError<String> as Type>::definition(types),
            doc: "Search things by query.",
            auth_mode: CoreAuthMode::None,
            mount_fn: stub_mount,
        }
    }

    inventory::submit! {
        teleport_core::ProcedureRegistration {
            module_path: "adapter_tests::search",
            fn_name: "healthCheck",
            prefix: None,
            method: CoreHttpMethod::Get,
            procedure_type: teleport_core::ProcedureType::Query,
            input_type: |types| <() as Type>::definition(types),
            output_type: |types| <Vec<String> as Type>::definition(types),
            error_type: |types| <() as Type>::definition(types),
            doc: "",
            auth_mode: CoreAuthMode::Optional,
            mount_fn: stub_mount,
        }
    }

    inventory::submit! {
        teleport_core::ProcedureRegistration {
            module_path: "adapter_tests::mutations",
            fn_name: "createThing",
            prefix: Some("admin"),
            method: CoreHttpMethod::Post,
            procedure_type: teleport_core::ProcedureType::Command,
            input_type: |types| <CreateInput as Type>::definition(types),
            output_type: |types| <String as Type>::definition(types),
            error_type: |types| <ProcedureError<String> as Type>::definition(types),
            doc: "Create a thing.",
            auth_mode: CoreAuthMode::Required,
            mount_fn: stub_mount,
        }
    }

    inventory::submit! {
        teleport_core::ProcedureRegistration {
            module_path: "adapter_tests::forms",
            fn_name: "submitThing",
            prefix: Some("forms"),
            method: CoreHttpMethod::Post,
            procedure_type: teleport_core::ProcedureType::Form,
            input_type: |types| <FormInput as Type>::definition(types),
            output_type: |types| <() as Type>::definition(types),
            error_type: |types| <ProcedureError<String> as Type>::definition(types),
            doc: "Submit a thing.",
            auth_mode: CoreAuthMode::Required,
            mount_fn: stub_mount,
        }
    }

    #[test]
    fn contract_from_inventory_adapts_routes_input_encodings_and_types() {
        let bundle = contract_from_inventory(&Config::new("generated").with_prefix("/api"))
            .expect("build contract");

        assert_eq!(bundle.version, teleport_contract::CONTRACT_VERSION);
        assert_eq!(
            bundle
                .procedures
                .iter()
                .map(|procedure| procedure.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "admin.createThing",
                "forms.submitThing",
                "search.healthCheck",
                "search.searchThings",
            ]
        );

        let create = bundle
            .procedures
            .iter()
            .find(|procedure| procedure.name == "admin.createThing")
            .expect("createThing procedure");
        assert_eq!(create.namespace, "admin");
        assert_eq!(create.method_name, "createThing");
        assert_eq!(create.procedure_kind, ProcedureKind::Command);
        assert_eq!(create.http_method, HttpMethod::Post);
        assert_eq!(create.path, "/api/admin.createThing");
        assert_eq!(create.input_encoding, InputEncoding::JsonBody);
        assert_eq!(create.auth_mode, AuthMode::Required);
        assert_eq!(create.doc, "Create a thing.");
        assert_eq!(create.output_type, TypeExpr::Primitive(PrimitiveType::str));

        let search = bundle
            .procedures
            .iter()
            .find(|procedure| procedure.name == "search.searchThings")
            .expect("searchThings procedure");
        assert_eq!(search.http_method, HttpMethod::Get);
        assert_eq!(search.input_encoding, InputEncoding::QueryString);
        assert_eq!(
            search.output_type,
            TypeExpr::Named {
                name: "GenericEnvelope".to_owned(),
                generics: vec![TypeExpr::Primitive(PrimitiveType::str)],
            }
        );

        let health = bundle
            .procedures
            .iter()
            .find(|procedure| procedure.name == "search.healthCheck")
            .expect("healthCheck procedure");
        assert_eq!(health.input_encoding, InputEncoding::None);
        assert_eq!(
            health.output_type,
            TypeExpr::List(Box::new(TypeExpr::Primitive(PrimitiveType::str)))
        );
        assert_eq!(health.auth_mode, AuthMode::Optional);

        let submit = bundle
            .procedures
            .iter()
            .find(|procedure| procedure.name == "forms.submitThing")
            .expect("submitThing procedure");
        assert_eq!(submit.input_encoding, InputEncoding::FormBody);
        assert_eq!(submit.output_type, TypeExpr::Tuple(Vec::new()));

        let generic_envelope = bundle
            .types
            .iter()
            .find(|named| named.name == "GenericEnvelope")
            .expect("GenericEnvelope type");
        assert_eq!(generic_envelope.generics, vec!["T"]);
        assert!(matches!(
            &generic_envelope.kind,
            NamedTypeKind::Struct(FieldsContract::Named(fields))
                if fields.iter().any(|field| field.name == "payload"
                    && field.ty == Some(TypeExpr::Generic("T".to_owned())))
                && fields.iter().any(|field| field.name == "maybe_payload"
                    && field.ty == Some(TypeExpr::Nullable(Box::new(TypeExpr::Generic("T".to_owned())))))
                && fields.iter().any(|field| field.name == "tags"
                    && field.ty == Some(TypeExpr::List(Box::new(TypeExpr::Primitive(PrimitiveType::str)))))
                && fields.iter().any(|field| field.name == "counts"
                    && field.ty == Some(TypeExpr::Map {
                        key: Box::new(TypeExpr::Primitive(PrimitiveType::str)),
                        value: Box::new(TypeExpr::Primitive(PrimitiveType::i32)),
                    }))
        ));

        let procedure_error = bundle
            .types
            .iter()
            .find(|named| named.name == "ProcedureError")
            .expect("ProcedureError type");
        assert_eq!(procedure_error.generics, vec!["T"]);
        assert!(matches!(
            &procedure_error.kind,
            NamedTypeKind::Enum(variants)
                if variants.iter().any(|variant| variant.name == "Idle"
                    && variant.fields == FieldsContract::Unit)
                && variants.iter().any(|variant| variant.name == "RetryAfter"
                    && variant.fields == FieldsContract::Unnamed(vec![UnnamedFieldContract {
                        docs: String::new(),
                        ty: Some(TypeExpr::Primitive(PrimitiveType::u32)),
                    }]))
                && variants.iter().any(|variant| variant.name == "Invalid"
                    && matches!(&variant.fields, FieldsContract::Named(fields)
                        if fields.iter().any(|field| field.name == "message"
                            && field.ty == Some(TypeExpr::Primitive(PrimitiveType::str)))
                        && fields.iter().any(|field| field.name == "context"
                            && field.ty == Some(TypeExpr::Nullable(Box::new(TypeExpr::Generic("T".to_owned())))))))
        ));

        assert!(
            bundle
                .types
                .iter()
                .all(|named| named.name != "Vec" && named.name != "String"),
            "stdlib wrappers should not leak into exported named types: {:?}",
            bundle
                .types
                .iter()
                .map(|named| named.name.as_str())
                .collect::<Vec<_>>()
        );
    }
}
