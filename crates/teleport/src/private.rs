// Internal module for macro-generated code. Not part of the public API.
//
// The `#[remote]` proc macro generates code that references types from this
// module. End users should never import from here.

pub use crate::procedure::{ErasedMountFn, HttpMethod, ProcedureRegistration, ProcedureType};
pub use inventory;

/// Extract the namespace (last segment) from a `module_path!()` string.
///
/// `"my_app::api::users"` → `"users"`
#[must_use]
pub fn namespace_from_module_path(module_path: &str) -> &str {
    module_path.rsplit("::").next().unwrap_or(module_path)
}

/// Convert `HttpMethod` to an `axum::routing::MethodFilter`.
#[must_use]
pub const fn to_method_filter(method: HttpMethod) -> axum::routing::MethodFilter {
    match method {
        HttpMethod::Get => axum::routing::MethodFilter::GET,
        HttpMethod::Post => axum::routing::MethodFilter::POST,
    }
}
