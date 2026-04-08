mod remote;
mod teleport_type;
mod utils;

use proc_macro::TokenStream;

/// Marks a function as a remote procedure callable from TypeScript.
///
/// # Procedure types
/// - `#[remote(query)]` — GET request, read-only
/// - `#[remote(command)]` — POST request, mutations
/// - `#[remote(form)]` — POST request, form submissions
#[proc_macro_attribute]
pub fn remote(attr: TokenStream, item: TokenStream) -> TokenStream {
    remote::expand(attr.into(), item.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Convenience attribute that expands to `Serialize + Deserialize + specta::Type`.
///
/// ```ignore
/// #[teleport_type]
/// pub struct LoginRequest {
///     pub email: String,
///     pub password: String,
/// }
/// ```
#[proc_macro_attribute]
pub fn teleport_type(attr: TokenStream, item: TokenStream) -> TokenStream {
    teleport_type::expand(attr.into(), item.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
