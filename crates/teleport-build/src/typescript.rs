use std::fmt::Write as _;

use teleport_contract::{ContractBundle, FieldsContract, NamedTypeKind};

use crate::GenerateError;

/// Generate the contents of `types.ts` from the contract bundle.
pub fn generate_types(bundle: &ContractBundle) -> Result<String, GenerateError> {
    let mut out = String::with_capacity(2048);
    out.push_str(crate::GENERATED_HEADER);
    out.push('\n');
    out.push('\n');

    for named in &bundle.types {
        match &named.kind {
            NamedTypeKind::Struct(fields) => {
                let generics = render_generics(&named.generics);
                let body = render_fields(fields);
                let _ = writeln!(out, "export type {}{} = {};", named.name, generics, body);
            }
            NamedTypeKind::Alias(ty) => {
                let generics = render_generics(&named.generics);
                let _ = writeln!(
                    out,
                    "export type {}{} = {};",
                    named.name,
                    generics,
                    crate::ts_utils::type_expr_to_ts(ty)
                );
            }
            NamedTypeKind::Enum(variants) => {
                let generics = render_generics(&named.generics);
                let _ = writeln!(out, "export type {}{} =", named.name, generics);
                for (idx, variant) in variants.iter().enumerate() {
                    let rendered = match &variant.fields {
                        FieldsContract::Unit => format!("\"{}\"", variant.name),
                        FieldsContract::Named(fields) => {
                            let body = if fields.is_empty() {
                                "null".to_owned()
                            } else {
                                format!(
                                    "{{ {} }}",
                                    fields
                                        .iter()
                                        .map(|field| {
                                            let optional = if field.optional { "?" } else { "" };
                                            let ty = field.ty.as_ref().map_or_else(
                                                || "never".to_owned(),
                                                crate::ts_utils::type_expr_to_ts,
                                            );
                                            format!("{}{}: {}", field.name, optional, ty)
                                        })
                                        .collect::<Vec<_>>()
                                        .join("; ")
                                )
                            };
                            format!("{{ {}: {} }}", variant.name, body)
                        }
                        FieldsContract::Unnamed(fields) => match fields.len() {
                            0 => format!("\"{}\"", variant.name),
                            1 => format!(
                                "{{ {}: {} }}",
                                variant.name,
                                fields[0].ty.as_ref().map_or_else(
                                    || "never".to_owned(),
                                    crate::ts_utils::type_expr_to_ts,
                                )
                            ),
                            _ => format!(
                                "{{ {}: [{}] }}",
                                variant.name,
                                fields
                                    .iter()
                                    .map(|field| {
                                        field.ty.as_ref().map_or_else(
                                            || "never".to_owned(),
                                            crate::ts_utils::type_expr_to_ts,
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        },
                    };
                    let suffix = if idx + 1 == variants.len() { ";" } else { "" };
                    let _ = writeln!(out, "\t| {rendered}{suffix}");
                }
            }
        }
        out.push('\n');
    }

    Ok(out)
}

fn render_generics(generics: &[String]) -> String {
    if generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", generics.join(", "))
    }
}

fn render_fields(fields: &FieldsContract) -> String {
    match fields {
        FieldsContract::Unit => "null".to_owned(),
        FieldsContract::Unnamed(fields) => {
            if fields.is_empty() {
                "[]".to_owned()
            } else {
                format!(
                    "[{}]",
                    fields
                        .iter()
                        .map(|field| {
                            field.ty.as_ref().map_or_else(
                                || "never".to_owned(),
                                crate::ts_utils::type_expr_to_ts,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        FieldsContract::Named(fields) => {
            if fields.is_empty() {
                "{}".to_owned()
            } else {
                format!(
                    "{{ {} }}",
                    fields
                        .iter()
                        .map(|field| {
                            let optional = if field.optional { "?" } else { "" };
                            let ty = field.ty.as_ref().map_or_else(
                                || "never".to_owned(),
                                crate::ts_utils::type_expr_to_ts,
                            );
                            format!("{}{}: {}", field.name, optional, ty)
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use teleport_contract::{
        ContractBundle, FieldsContract, NamedFieldContract, NamedTypeContract, NamedTypeKind,
        PrimitiveType, TypeExpr, UnnamedFieldContract, VariantContract,
    };

    use super::generate_types;

    #[test]
    fn renders_struct_alias_and_enum_shapes_across_edge_cases() {
        let bundle = ContractBundle {
            version: teleport_contract::CONTRACT_VERSION.to_owned(),
            procedures: Vec::new(),
            types: vec![
                NamedTypeContract {
                    name: "Envelope".to_owned(),
                    docs: String::new(),
                    generics: vec!["T".to_owned()],
                    kind: NamedTypeKind::Struct(FieldsContract::Named(vec![
                        NamedFieldContract {
                            name: "payload".to_owned(),
                            docs: String::new(),
                            optional: false,
                            ty: Some(TypeExpr::Generic("T".to_owned())),
                        },
                        NamedFieldContract {
                            name: "fallback".to_owned(),
                            docs: String::new(),
                            optional: true,
                            ty: None,
                        },
                    ])),
                },
                NamedTypeContract {
                    name: "TuplePayload".to_owned(),
                    docs: String::new(),
                    generics: Vec::new(),
                    kind: NamedTypeKind::Struct(FieldsContract::Unnamed(vec![
                        UnnamedFieldContract {
                            docs: String::new(),
                            ty: Some(TypeExpr::Primitive(PrimitiveType::bool)),
                        },
                        UnnamedFieldContract {
                            docs: String::new(),
                            ty: Some(TypeExpr::Primitive(PrimitiveType::str)),
                        },
                    ])),
                },
                NamedTypeContract {
                    name: "EmptyTuple".to_owned(),
                    docs: String::new(),
                    generics: Vec::new(),
                    kind: NamedTypeKind::Struct(FieldsContract::Unnamed(Vec::new())),
                },
                NamedTypeContract {
                    name: "Boxed".to_owned(),
                    docs: String::new(),
                    generics: vec!["T".to_owned()],
                    kind: NamedTypeKind::Alias(TypeExpr::List(Box::new(TypeExpr::Generic(
                        "T".to_owned(),
                    )))),
                },
                NamedTypeContract {
                    name: "RemoteState".to_owned(),
                    docs: String::new(),
                    generics: vec!["T".to_owned()],
                    kind: NamedTypeKind::Enum(vec![
                        VariantContract {
                            name: "Idle".to_owned(),
                            docs: String::new(),
                            fields: FieldsContract::Unit,
                        },
                        VariantContract {
                            name: "Empty".to_owned(),
                            docs: String::new(),
                            fields: FieldsContract::Named(Vec::new()),
                        },
                        VariantContract {
                            name: "Ready".to_owned(),
                            docs: String::new(),
                            fields: FieldsContract::Named(vec![
                                NamedFieldContract {
                                    name: "value".to_owned(),
                                    docs: String::new(),
                                    optional: false,
                                    ty: Some(TypeExpr::Generic("T".to_owned())),
                                },
                                NamedFieldContract {
                                    name: "meta".to_owned(),
                                    docs: String::new(),
                                    optional: true,
                                    ty: None,
                                },
                            ]),
                        },
                        VariantContract {
                            name: "Nothing".to_owned(),
                            docs: String::new(),
                            fields: FieldsContract::Unnamed(Vec::new()),
                        },
                        VariantContract {
                            name: "Value".to_owned(),
                            docs: String::new(),
                            fields: FieldsContract::Unnamed(vec![UnnamedFieldContract {
                                docs: String::new(),
                                ty: Some(TypeExpr::Generic("T".to_owned())),
                            }]),
                        },
                        VariantContract {
                            name: "Pair".to_owned(),
                            docs: String::new(),
                            fields: FieldsContract::Unnamed(vec![
                                UnnamedFieldContract {
                                    docs: String::new(),
                                    ty: Some(TypeExpr::Generic("T".to_owned())),
                                },
                                UnnamedFieldContract {
                                    docs: String::new(),
                                    ty: Some(TypeExpr::Primitive(PrimitiveType::str)),
                                },
                            ]),
                        },
                    ]),
                },
            ],
        };

        let output = generate_types(&bundle).expect("generate types");

        assert!(output.starts_with(crate::GENERATED_HEADER));
        assert!(output.contains("export type Envelope<T> = { payload: T; fallback?: never };"));
        assert!(output.contains("export type TuplePayload = [boolean, string];"));
        assert!(output.contains("export type EmptyTuple = [];"));
        assert!(output.contains("export type Boxed<T> = T[];"));
        assert!(output.contains("export type RemoteState<T> ="));
        assert!(output.contains("\t| \"Idle\""));
        assert!(output.contains("\t| { Empty: null }"));
        assert!(output.contains("\t| { Ready: { value: T; meta?: never } }"));
        assert!(output.contains("\t| \"Nothing\""));
        assert!(output.contains("\t| { Value: T }"));
        assert!(output.contains("\t| { Pair: [T, string] };"));
    }
}
