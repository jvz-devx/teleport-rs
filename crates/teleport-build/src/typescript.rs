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
    let raw = ts
        .export(&rewritten)
        .map_err(|e| GenerateError::TypeExport(e.to_string()))?;

    // Post-process: fix externally-tagged enums with struct variants.
    // `specta-typescript` 0.0.11 has the correct renderer for these
    // commented out and falls back to a legacy path that collapses
    // variant names into a single union — see `rewrite_enum_with_struct_variants`.
    Ok(rewrite_enums_with_struct_variants(&raw, &rewritten))
}

/// Walk the resolved type collection, find every enum that has at least
/// one non-unit variant, and replace specta-typescript's broken rendering
/// in `raw` with the correct externally-tagged form.
///
/// See `docs/error-handling.md` §"Detail type constraints" for the bug
/// this works around.
fn rewrite_enums_with_struct_variants(raw: &str, types: &ResolvedTypes) -> String {
    use specta::datatype::{DataType, Fields};

    let mut output = raw.to_owned();

    for ndt in types.as_types().into_sorted_iter() {
        let DataType::Enum(enum_dt) = ndt.ty() else {
            continue;
        };
        // Skip enums that only have unit variants — specta renders those
        // correctly as a string literal union.
        let has_fielded_variant = enum_dt
            .variants()
            .iter()
            .any(|(_, v)| !matches!(v.fields(), Fields::Unit));
        if !has_fielded_variant {
            continue;
        }

        let enum_name = ndt.name();
        let Some(correct) = render_enum_externally_tagged(enum_name.as_ref(), enum_dt, types)
        else {
            continue;
        };

        if let Some(replaced) = replace_enum_definition(&output, enum_name.as_ref(), &correct) {
            output = replaced;
        }
    }

    output
}

/// Render an enum as the correct externally-tagged TypeScript type:
///
/// ```text
/// export type EnumName =
///     | "UnitVariant"
///     | { StructVariant: { field: string } }
///     | { TupleVariant: [string, number] };
/// ```
///
/// Returns `None` if any variant references a type that `datatype_to_ts`
/// cannot render (which would leave us with a malformed replacement).
fn render_enum_externally_tagged(
    enum_name: &str,
    enum_dt: &specta::datatype::Enum,
    resolved_types: &ResolvedTypes,
) -> Option<String> {
    use specta::datatype::Fields;

    let exporter = Typescript::default();
    let mut lines: Vec<String> = Vec::with_capacity(enum_dt.variants().len());

    for (variant_name, variant) in enum_dt.variants() {
        let line = match variant.fields() {
            Fields::Unit => format!(r#""{variant_name}""#),
            Fields::Named(named) => {
                let mut parts = Vec::new();
                for (field_name, field) in named.fields() {
                    let Some(ty) = field.ty() else { continue };
                    let ts = crate::ts_utils::datatype_to_ts(&exporter, resolved_types, ty).ok()?;
                    parts.push(format!("{field_name}: {ts}"));
                }
                if parts.is_empty() {
                    format!(r#""{variant_name}""#)
                } else {
                    format!("{{ {variant_name}: {{ {} }} }}", parts.join(", "))
                }
            }
            Fields::Unnamed(unnamed) => {
                let field_types: Vec<String> = unnamed
                    .fields()
                    .iter()
                    .filter_map(|f| f.ty())
                    .map(|ty| crate::ts_utils::datatype_to_ts(&exporter, resolved_types, ty))
                    .collect::<Result<_, _>>()
                    .ok()?;
                match field_types.len() {
                    0 => format!(r#""{variant_name}""#),
                    1 => format!("{{ {variant_name}: {} }}", field_types[0]),
                    _ => format!("{{ {variant_name}: [{}] }}", field_types.join(", ")),
                }
            }
        };
        lines.push(line);
    }

    if lines.is_empty() {
        return Some(format!("export type {enum_name} = never;"));
    }

    let body = lines
        .iter()
        .map(|l| format!("\t| {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!("export type {enum_name} =\n{body};"))
}

/// Replace the existing `export type <enum_name> = …;` block in `output`
/// with `replacement`. Searches line by line for the start marker and
/// consumes lines until it finds the terminating `;` (which for the
/// broken specta output is always on the same line).
///
/// Returns `None` if the definition could not be located.
fn replace_enum_definition(output: &str, enum_name: &str, replacement: &str) -> Option<String> {
    let needle = format!("export type {enum_name} = ");
    let start = output.find(&needle)?;
    // Specta 0.0.11's legacy enum renderer always produces the whole
    // definition on a single line ending in `;`. Find that `;` relative
    // to the match.
    let rest = &output[start..];
    let end_rel = rest.find(";\n").map(|p| p + 2).or_else(|| {
        // Tolerate a missing trailing newline (end of file).
        rest.find(';').map(|p| p + 1)
    })?;
    let end = start + end_rel;

    let mut rewritten = String::with_capacity(output.len());
    rewritten.push_str(&output[..start]);
    rewritten.push_str(replacement);
    rewritten.push('\n');
    rewritten.push_str(&output[end..]);
    Some(rewritten)
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
