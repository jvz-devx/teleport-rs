use proc_macro2::TokenStream;
use quote::quote;
use syn::{Item, Result, parse2};

/// Expand the `#[teleport_type]` attribute macro.
///
/// Prepends `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]`
/// to the annotated struct or enum.
pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "`#[teleport_type]` does not accept arguments",
        ));
    }

    let item: Item = parse2(item)?;

    match &item {
        Item::Struct(_) | Item::Enum(_) => {}
        _ => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "`#[teleport_type]` can only be applied to structs and enums",
            ));
        }
    }

    Ok(quote! {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
        #item
    })
}
