mod remote;
mod teleport_type;
mod utils;

use proc_macro::TokenStream;

/// Marks a function as a remote procedure callable from TypeScript.
///
/// # Procedure types
///
/// - `#[remote(query)]` — GET request, input from query string, read-only
/// - `#[remote(command)]` — POST request, input from JSON body, mutations
/// - `#[remote(form)]` — POST request, input from form-urlencoded or JSON
///
/// # Attribute keys
///
/// In addition to the procedure type, `#[remote]` supports two optional
/// name-value keys that override how the procedure appears in the
/// generated TypeScript client:
///
/// - `prefix = "..."` — replaces the default module-path namespace. By
///   default, a procedure's TS name is `{module_path}.{fn_name}` where
///   `module_path` is the last segment of `module_path!()`. For a
///   procedure in `src/api/users.rs`, that works out to `users.getUser`
///   — exactly what you want. For a single-file app where the function
///   lives in `main.rs`, the module is the crate name, so you'd get
///   `my_app.getUser`. Override with `prefix = "users"` to force the
///   namespace.
/// - `name = "..."` — replaces the default `camelCase(fn_ident)` name.
///   Use this when the Rust identifier is ugly or collides.
///
/// ```ignore
/// // Default: namespace derives from module path.
/// #[remote(query)]
/// async fn get_user(ctx: &AppState, input: GetUserInput) -> Result<User, AppError> { /* ... */ }
///
/// // Explicit override: appears as `users.getUser` regardless of file location.
/// #[remote(query, prefix = "users", name = "getUser")]
/// async fn get_user(ctx: &AppState, input: GetUserInput) -> Result<User, AppError> { /* ... */ }
/// ```
///
/// # Signature requirements
///
/// The function must be `async`, take `&S` (a reference to your state
/// type) as its first parameter, take at most one additional `input`
/// parameter (which must be a `#[teleport_type]` struct — bare
/// primitives are not supported because `serde_qs` cannot deserialize
/// them from a query string), and return `Result<T, AppError<E>>`
/// where `T` and `E` are `#[teleport_type]`.
#[proc_macro_attribute]
pub fn remote(attr: TokenStream, item: TokenStream) -> TokenStream {
    remote::expand(attr.into(), item.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Convenience attribute for types that cross the Rust ↔ TypeScript
/// boundary via teleport-rs.
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
/// (conflicting implementations). If you need additional derives like
/// `PartialEq`, `Eq`, or `Hash`, add them on a *separate*
/// `#[derive(...)]` line alongside `#[teleport_type]`.
///
/// ```ignore
/// // Correct — no duplicate derives.
/// #[teleport_type]
/// #[derive(PartialEq, Eq, Hash)]
/// pub struct LoginRequest {
///     pub email: String,
///     pub password: String,
/// }
/// ```
///
/// ```compile_fail,ignore
/// // E0119 — `Clone` is already derived by `#[teleport_type]`.
/// #[teleport_type]
/// #[derive(Clone)]
/// pub struct LoginRequest { /* ... */ }
/// ```
///
/// # Supported shapes
///
/// Structs with named fields, structs with a single tuple field
/// (newtypes), and enums are all supported. Tuple structs with more
/// than one field and unit structs produce less predictable TypeScript
/// — prefer named-field structs when possible.
#[proc_macro_attribute]
pub fn teleport_type(attr: TokenStream, item: TokenStream) -> TokenStream {
    teleport_type::expand(attr.into(), item.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
