// Specta DataType -> TypeScript type generation.
//
// Converts collected procedure types into `types.ts` content.

use specta::ResolvedTypes;
use specta::datatype::{DataType, Primitive};
use specta_typescript::Typescript;

use crate::GenerateError;

/// Generate the contents of `types.ts` from the resolved type collection.
///
/// Uses `specta-typescript` to render all named types (structs, enums)
/// that were registered by the export binary.
///
/// # `BigInt` handling
///
/// `specta-typescript` 0.0.11 has no knob for the `BigInt` forbid policy,
/// and flat-out refuses to render `i64`, `u64`, `i128`, `u128`, `isize`,
/// and `usize` primitives because representing them as JS `number` would
/// silently lose precision for values greater than 2^53. That behaviour
/// previously bubbled up as a raw specta error with an empty type name,
/// giving users no hint which procedure or struct field was at fault.
///
/// To keep 0.1.x ergonomic for the common case of "timestamp / id /
/// duration as `i64`", we walk the type collection before handing it to
/// specta and rewrite every 64-bit integer primitive to [`Primitive::str`].
/// Downstream specta renders `str` as the TS `string` type, which matches
/// the JSON wire format when the Rust side serialises integers as JSON
/// strings (the standard serde workaround for >2^53 values).
///
/// The matching rewrite lives in `crate::ts_utils::datatype_to_ts` for
/// top-level procedure input / output / error types (which are direct
/// `DataType`s not stored in `Types`).
pub fn generate_types(resolved_types: &ResolvedTypes) -> Result<String, GenerateError> {
    let rewritten = rewrite_bigint_to_string(resolved_types);

    let ts = Typescript::default().header(crate::GENERATED_HEADER);
    ts.export(&rewritten)
        .map_err(|e| GenerateError::TypeExport(e.to_string()))
}

/// Clone `resolved_types` and rewrite every `i64` / `u64` / `i128` /
/// `u128` / `isize` / `usize` primitive to `str` so that
/// `specta-typescript` will render the containing struct field as
/// `string` instead of aborting the export. See [`generate_types`] for
/// the rationale.
fn rewrite_bigint_to_string(resolved_types: &ResolvedTypes) -> ResolvedTypes {
    let mut types = resolved_types.as_types().clone();
    types.iter_mut(|ndt| rewrite_dt(ndt.ty_mut()));
    ResolvedTypes::from_resolved_types(types)
}

/// Recursively rewrite `i64`/`u64`/`i128`/`u128`/`isize`/`usize`
/// primitives to `str` inside a `DataType`. Other variants are walked
/// structurally so that rewrites nested inside lists, maps, tuples,
/// struct fields, and enum variants are all caught.
fn rewrite_dt(dt: &mut DataType) {
    match dt {
        DataType::Primitive(p)
            if matches!(
                p,
                Primitive::i64
                    | Primitive::u64
                    | Primitive::i128
                    | Primitive::u128
                    | Primitive::isize
                    | Primitive::usize
            ) =>
        {
            *p = Primitive::str;
        }
        DataType::Primitive(_) => {}
        DataType::List(list) => rewrite_dt(list.ty_mut()),
        DataType::Map(map) => {
            rewrite_dt(map.key_ty_mut());
            rewrite_dt(map.value_ty_mut());
        }
        DataType::Tuple(tuple) => {
            for el in tuple.elements_mut() {
                rewrite_dt(el);
            }
        }
        DataType::Nullable(inner) => rewrite_dt(inner),
        DataType::Struct(s) => rewrite_fields(s.fields_mut()),
        DataType::Enum(e) => {
            for (_name, variant) in e.variants_mut() {
                rewrite_fields(variant.fields_mut());
            }
        }
        DataType::Reference(reference) => {
            // References point at other NamedDataTypes in the collection,
            // which are walked independently via `Types::iter_mut`. We
            // don't need to recurse through the referenced definition
            // here — only the `Reference`'s own generic-argument slots
            // carry inline `DataType`s that won't be visited otherwise.
            if let specta::datatype::Reference::Named(named_ref) = reference {
                for (_g, arg_dt) in named_ref.generics_mut() {
                    rewrite_dt(arg_dt);
                }
            }
        }
    }
}

fn rewrite_fields(fields: &mut specta::datatype::Fields) {
    use specta::datatype::Fields;
    match fields {
        Fields::Named(named) => {
            for (_name, field) in named.fields_mut() {
                if let Some(ty) = field.ty_mut() {
                    rewrite_dt(ty);
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            for field in unnamed.fields_mut() {
                if let Some(ty) = field.ty_mut() {
                    rewrite_dt(ty);
                }
            }
        }
        Fields::Unit => {}
    }
}

#[cfg(test)]
mod tests {
    // Test-only: `.expect()` is informative and any panic is caught by the test runner.
    #![allow(clippy::expect_used)]

    use super::*;
    use specta::Types;

    #[derive(Debug, Clone, specta::Type)]
    #[allow(dead_code)]
    struct TestUser {
        id: String,
        name: String,
        email: String,
    }

    #[test]
    fn generates_interface_for_struct() {
        let types = Types::default().register::<TestUser>();
        let resolved = ResolvedTypes::from_resolved_types(types);
        let output = generate_types(&resolved).expect("should generate types");

        assert!(output.contains("export type TestUser"));
        assert!(output.contains("id"));
        assert!(output.contains("name"));
        assert!(output.contains("email"));
        assert!(output.contains(crate::GENERATED_HEADER));
    }

    #[test]
    fn empty_types_produces_header_only() {
        let types = Types::default();
        let resolved = ResolvedTypes::from_resolved_types(types);
        let output = generate_types(&resolved).expect("should generate types");

        assert!(output.contains(crate::GENERATED_HEADER));
    }
}
