// Shared utilities for converting `specta::DataType` to TypeScript strings.
//
// The key difference from `specta_typescript::primitives::inline` is that
// this always emits named type *references* (e.g. `User`) rather than
// inlining the struct body, because the full definitions live in `types.ts`.

use specta::ResolvedTypes;
use specta::datatype::{DataType, Primitive, Reference};

use crate::GenerateError;

/// Convert a `DataType` to its TypeScript string representation, preferring
/// named type references over inline struct definitions.
///
/// Named types are rendered as their type name (e.g. `"User"`), while
/// primitives, tuples, lists, and other anonymous types are rendered inline.
///
/// Rust stdlib wrappers that specta registers as `NamedDataType` but that
/// have no corresponding TypeScript type (`Vec<T>`, `String`, `HashMap`,
/// `HashSet`, …) are translated to inline TS constructs (`T[]`, `string`,
/// `Record<K, V>`, …). This prevents them from leaking into the import
/// list in the generated `client.ts` / `errors.ts`.
///
/// 64-bit integer primitives (`i64`, `u64`, `i128`, `u128`, `isize`,
/// `usize`) are rendered as `"string"` because specta-typescript's
/// primitive renderer refuses to emit them (to avoid silent precision
/// loss in JS `number`). The JSON wire format still carries the value as
/// a string, so the TS type matches the runtime shape.
pub fn datatype_to_ts(
    exporter: &specta_typescript::Typescript,
    resolved_types: &ResolvedTypes,
    dt: &DataType,
) -> Result<String, GenerateError> {
    match dt {
        // 64-bit integers: specta-typescript refuses these. Render as
        // `string` to match the JSON wire format (see types.rs rewrite).
        DataType::Primitive(
            Primitive::i64
            | Primitive::u64
            | Primitive::i128
            | Primitive::u128
            | Primitive::isize
            | Primitive::usize,
        ) => Ok("string".to_owned()),
        DataType::Reference(Reference::Named(named_ref)) => {
            // If the named type exists in the collection, look it up so we
            // can special-case stdlib wrappers before falling through to
            // the generic named-reference renderer.
            if let Some(ndt) = named_ref.get(resolved_types.as_types()) {
                let base_name = ndt.name();
                let generics = named_ref.generics();

                // Stdlib wrappers: render the inline TS construct instead
                // of emitting `Vec<Todo>` or `String` (which would then
                // leak into the import list).
                match base_name.as_ref() {
                    // `Vec<T>`, `VecDeque<T>`, `HashSet<T>`, `BTreeSet<T>`
                    // all map to `T[]`. HashSet/BTreeSet are grouped here
                    // because specta normalises them to a unique `List`.
                    "Vec" | "VecDeque" | "HashSet" | "BTreeSet" | "LinkedList" | "BinaryHeap" => {
                        if let Some((_g, arg_dt)) = generics.first() {
                            let elem_ts = datatype_to_ts(exporter, resolved_types, arg_dt)?;
                            return Ok(if elem_ts.contains('|') {
                                format!("({elem_ts})[]")
                            } else {
                                format!("{elem_ts}[]")
                            });
                        }
                    }
                    "String" | "str" => return Ok("string".to_owned()),
                    "HashMap" | "BTreeMap" => {
                        if generics.len() >= 2 {
                            let key_ts = datatype_to_ts(exporter, resolved_types, &generics[0].1)?;
                            let val_ts = datatype_to_ts(exporter, resolved_types, &generics[1].1)?;
                            return Ok(format!("Record<{key_ts}, {val_ts}>"));
                        }
                    }
                    _ => {}
                }

                let base_name = base_name.to_string();
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

/// Returns `true` for stdlib wrapper type names that specta registers as
/// named references but that [`datatype_to_ts`] translates to inline TS
/// constructs (e.g. `Vec<T>` → `T[]`, `String` → `string`).
///
/// Defence-in-depth: callers that scan a `DataType` graph for import
/// names use this to filter out stdlib wrapper names even if the name
/// somehow made it through. These must NOT appear in `import type { … }`
/// lines because they are never emitted as named exports in `types.ts`.
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
