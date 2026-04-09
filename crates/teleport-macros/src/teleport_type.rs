use proc_macro2::TokenStream;
use quote::quote;
use syn::{Item, Result, parse2};

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

    let item: Item = parse2(item)?;

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
