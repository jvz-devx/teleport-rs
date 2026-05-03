use std::collections::BTreeSet;

use teleport_contract::{PrimitiveType, TypeExpr};

/// Convert a contract type expression to TypeScript.
pub fn type_expr_to_ts(dt: &TypeExpr) -> String {
    match dt {
        TypeExpr::Primitive(primitive) => primitive_to_ts(*primitive).to_owned(),
        TypeExpr::List(inner) => {
            let elem = type_expr_to_ts(inner);
            if elem.contains('|') {
                format!("({elem})[]")
            } else {
                format!("{elem}[]")
            }
        }
        TypeExpr::Map { key, value } => {
            format!(
                "Record<{}, {}>",
                type_expr_to_ts(key),
                type_expr_to_ts(value)
            )
        }
        TypeExpr::Tuple(elements) => {
            if elements.is_empty() {
                "null".to_owned()
            } else {
                format!(
                    "[{}]",
                    elements
                        .iter()
                        .map(type_expr_to_ts)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        TypeExpr::Nullable(inner) => format!("{} | null", type_expr_to_ts(inner)),
        TypeExpr::Named { name, generics } => {
            if generics.is_empty() {
                name.clone()
            } else {
                format!(
                    "{name}<{}>",
                    generics
                        .iter()
                        .map(type_expr_to_ts)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        TypeExpr::Generic(name) => name.clone(),
        TypeExpr::Opaque(_) => "unknown".to_owned(),
    }
}

/// Recursively collect named type references from a type expression.
pub fn collect_type_names(dt: &TypeExpr, names: &mut BTreeSet<String>) {
    match dt {
        TypeExpr::Named { name, generics } => {
            names.insert(name.clone());
            for generic in generics {
                collect_type_names(generic, names);
            }
        }
        TypeExpr::List(inner) | TypeExpr::Nullable(inner) => collect_type_names(inner, names),
        TypeExpr::Map { key, value } => {
            collect_type_names(key, names);
            collect_type_names(value, names);
        }
        TypeExpr::Tuple(elements) => {
            for element in elements {
                collect_type_names(element, names);
            }
        }
        TypeExpr::Primitive(_) | TypeExpr::Generic(_) | TypeExpr::Opaque(_) => {}
    }
}

pub fn is_ts_primitive(ts_type: &str) -> bool {
    matches!(
        ts_type,
        "string"
            | "number"
            | "boolean"
            | "null"
            | "undefined"
            | "void"
            | "never"
            | "any"
            | "unknown"
    )
}

pub fn is_ts_stdlib_wrapper(name: &str) -> bool {
    matches!(
        name,
        "Vec"
            | "VecDeque"
            | "LinkedList"
            | "BinaryHeap"
            | "HashSet"
            | "BTreeSet"
            | "HashMap"
            | "BTreeMap"
            | "String"
            | "str"
    )
}

const fn primitive_to_ts(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::i8
        | PrimitiveType::i16
        | PrimitiveType::i32
        | PrimitiveType::u8
        | PrimitiveType::u16
        | PrimitiveType::u32
        | PrimitiveType::f16
        | PrimitiveType::f32
        | PrimitiveType::f64
        | PrimitiveType::f128 => "number",
        PrimitiveType::i64
        | PrimitiveType::i128
        | PrimitiveType::isize
        | PrimitiveType::u64
        | PrimitiveType::u128
        | PrimitiveType::usize
        | PrimitiveType::char
        | PrimitiveType::str => "string",
        PrimitiveType::bool => "boolean",
    }
}
