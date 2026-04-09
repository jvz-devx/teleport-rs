use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Field, Fields, Item, Result, Type, parse_quote, parse2};

/// Expand the `#[teleport_type]` attribute macro.
///
/// Prepends `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]`
/// to the annotated struct or enum.
///
/// # Expands to
///
/// `#[teleport_type]` adds these derives automatically:
///
/// - `Debug`
/// - `Clone`
/// - `serde::Serialize`
/// - `serde::Deserialize`
/// - `specta::Type`
///
/// **Do NOT add any of these derives yourself** — you will hit E0119
/// (conflicting implementations). If you need `PartialEq`, `Eq`, `Hash`,
/// etc., add those as a separate `#[derive(...)]` line alongside
/// `#[teleport_type]`:
///
/// ```ignore
/// #[teleport_type]
/// #[derive(PartialEq, Eq, Hash)]
/// pub struct UserId(pub String);
/// ```
pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "`#[teleport_type]` does not accept arguments\n  \
             write `#[teleport_type]` on its own, directly above the struct or enum",
        ));
    }

    let mut item: Item = parse2(item)?;

    match &mut item {
        Item::Struct(s) => {
            inject_bigint_attrs(&mut s.fields);
        }
        Item::Enum(e) => {
            for variant in &mut e.variants {
                inject_bigint_attrs(&mut variant.fields);
            }
        }
        _ => {}
    }

    if matches!(item, Item::Struct(_) | Item::Enum(_)) {
        return Ok(quote! {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
            #item
        });
    }

    let kind = match &item {
        Item::Fn(_) => "a function",
        Item::Impl(_) => "an impl block",
        Item::Mod(_) => "a module",
        Item::Trait(_) => "a trait",
        Item::Type(_) => "a type alias",
        Item::Union(_) => "a union",
        Item::Const(_) => "a const",
        Item::Static(_) => "a static",
        _ => "this item",
    };

    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        format!(
            "`#[teleport_type]` can only be applied to structs or enums, not {kind}\n  \
             move the attribute to a struct or enum definition that should be exposed to TypeScript"
        ),
    ))
}

/// Walk struct/variant fields and, for any field whose type is a 64-bit
/// integer primitive (`i64` / `u64` / `i128` / `u128` / `isize` / `usize`)
/// or `Option<T>` thereof, inject a `#[serde(with = "…")]` attribute
/// that forces the Rust side to serialise the value as a JSON string.
///
/// This keeps the Rust wire format and the TypeScript type in sync:
/// `teleport-build` already rewrites the specta `Primitive::i64` to
/// render as TypeScript `string`, but without the serde attribute the
/// Rust side would still emit a JSON number. JavaScript's `number`
/// silently loses precision above 2^53, so the correct long-term
/// representation is a JSON string on both sides.
///
/// Fields that already carry their own `#[serde(with = …)]` are left
/// untouched — user overrides take precedence.
fn inject_bigint_attrs(fields: &mut Fields) {
    let field_list: &mut syn::punctuated::Punctuated<Field, syn::token::Comma> = match fields {
        Fields::Named(named) => &mut named.named,
        Fields::Unnamed(unnamed) => &mut unnamed.unnamed,
        Fields::Unit => return,
    };
    for field in field_list.iter_mut() {
        if has_serde_with_attr(&field.attrs) {
            continue;
        }
        if let Some(path) = bigint_serde_module_path(&field.ty) {
            let attr: Attribute = parse_quote! { #[serde(with = #path)] };
            field.attrs.push(attr);
        }
    }
}

/// Detect `#[serde(with = …)]` on a field's existing attributes. Does a
/// simple substring search because the `Meta::List` token stream has
/// variable whitespace and we only want to avoid overriding explicit
/// user choices — false positives just mean we skip an auto-injection.
fn has_serde_with_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("serde") {
            return false;
        }
        if let syn::Meta::List(list) = &a.meta {
            return list.tokens.to_string().contains("with");
        }
        false
    })
}

/// Map a field type to the fully-qualified path of a serde helper
/// module that serialises it as a JSON string. Returns `None` for any
/// type we don't auto-handle (plain `i32`, `String`, user structs,
/// `Vec<i64>`, `HashMap<_, i64>` — these keep default serde behaviour).
fn bigint_serde_module_path(ty: &Type) -> Option<&'static str> {
    let Type::Path(tp) = ty else {
        return None;
    };
    if tp.qself.is_some() || tp.path.segments.len() != 1 {
        return None;
    }
    let seg = &tp.path.segments[0];
    let ident = seg.ident.to_string();

    // Plain 64-bit primitive field.
    match ident.as_str() {
        "i64" => return Some("::teleport::bigint::i64_as_string"),
        "u64" => return Some("::teleport::bigint::u64_as_string"),
        "i128" => return Some("::teleport::bigint::i128_as_string"),
        "u128" => return Some("::teleport::bigint::u128_as_string"),
        "isize" => return Some("::teleport::bigint::isize_as_string"),
        "usize" => return Some("::teleport::bigint::usize_as_string"),
        _ => {}
    }

    // `Option<T>` where T is a 64-bit primitive.
    if ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    let Some(syn::GenericArgument::Type(Type::Path(inner_tp))) = args.args.first() else {
        return None;
    };
    if inner_tp.qself.is_some() || inner_tp.path.segments.len() != 1 {
        return None;
    }
    let inner_ident = inner_tp.path.segments[0].ident.to_string();
    match inner_ident.as_str() {
        "i64" => Some("::teleport::bigint::opt_i64_as_string"),
        "u64" => Some("::teleport::bigint::opt_u64_as_string"),
        "i128" => Some("::teleport::bigint::opt_i128_as_string"),
        "u128" => Some("::teleport::bigint::opt_u128_as_string"),
        "isize" => Some("::teleport::bigint::opt_isize_as_string"),
        "usize" => Some("::teleport::bigint::opt_usize_as_string"),
        _ => None,
    }
}
