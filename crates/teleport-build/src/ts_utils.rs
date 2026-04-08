// Shared utilities for converting `specta::DataType` to TypeScript strings.
//
// The key difference from `specta_typescript::primitives::inline` is that
// this always emits named type *references* (e.g. `User`) rather than
// inlining the struct body, because the full definitions live in `types.ts`.

use specta::ResolvedTypes;
use specta::datatype::{DataType, Reference};

use crate::GenerateError;

/// Convert a `DataType` to its TypeScript string representation, preferring
/// named type references over inline struct definitions.
///
/// Named types are rendered as their type name (e.g. `"User"`), while
/// primitives, tuples, lists, and other anonymous types are rendered inline.
pub fn datatype_to_ts(
    exporter: &specta_typescript::Typescript,
    resolved_types: &ResolvedTypes,
    dt: &DataType,
) -> Result<String, GenerateError> {
    match dt {
        DataType::Reference(Reference::Named(named_ref)) => {
            // If the named type exists in the collection, use its name.
            if let Some(ndt) = named_ref.get(resolved_types.as_types()) {
                let base_name = ndt.name().to_string();
                let generics = named_ref.generics();
                if generics.is_empty() {
                    return Ok(base_name);
                }
                // Render generic arguments: `Result<User, Error>` etc.
                let args: Vec<String> = generics
                    .iter()
                    .map(|(_g, arg_dt)| datatype_to_ts(exporter, resolved_types, arg_dt))
                    .collect::<Result<_, _>>()?;
                return Ok(format!("{base_name}<{}>", args.join(", ")));
            }
            // Fall back to specta-typescript's inline renderer.
            specta_typescript::primitives::inline(exporter, resolved_types, dt)
                .map_err(|e| GenerateError::TypeExport(e.to_string()))
        }
        DataType::Nullable(inner) => {
            let inner_ts = datatype_to_ts(exporter, resolved_types, inner)?;
            Ok(format!("{inner_ts} | null"))
        }
        DataType::List(list) => {
            let elem_ts = datatype_to_ts(exporter, resolved_types, list.ty())?;
            // Wrap union types in parens: `(A | B)[]`
            if elem_ts.contains('|') {
                Ok(format!("({elem_ts})[]"))
            } else {
                Ok(format!("{elem_ts}[]"))
            }
        }
        DataType::Map(map) => {
            let key_ts = datatype_to_ts(exporter, resolved_types, map.key_ty())?;
            let val_ts = datatype_to_ts(exporter, resolved_types, map.value_ty())?;
            Ok(format!("Record<{key_ts}, {val_ts}>"))
        }
        DataType::Tuple(tuple) => {
            let elements = tuple.elements();
            if elements.is_empty() {
                return Ok("null".to_owned());
            }
            let parts: Vec<String> = elements
                .iter()
                .map(|el| datatype_to_ts(exporter, resolved_types, el))
                .collect::<Result<_, _>>()?;
            Ok(format!("[{}]", parts.join(", ")))
        }
        // Primitives, opaque references (never/any/unknown), enums, structs.
        _ => specta_typescript::primitives::inline(exporter, resolved_types, dt)
            .map_err(|e| GenerateError::TypeExport(e.to_string())),
    }
}

/// Returns `true` for TS built-in type names that do not need importing.
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
