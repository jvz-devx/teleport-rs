use proc_macro2::TokenStream;
use syn::Result;

/// Expand the `#[remote(query|command|form)]` attribute macro.
///
/// Currently a pass-through stub. Phase 1 implementation will:
/// - Parse the procedure type from the attribute
/// - Validate the function signature
/// - Generate an `inventory::submit!` block with procedure metadata
/// - Generate an Axum handler wrapper
// Will return errors once signature validation is implemented.
#[allow(clippy::unnecessary_wraps)]
pub fn expand(_attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    // Stub: pass through the original function unchanged.
    Ok(item)
}
