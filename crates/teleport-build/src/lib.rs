pub mod client;
pub mod config;
pub mod errors;
pub mod naming;
pub mod typescript;

pub use config::{Config, Naming, NamingCase, NamespaceStyle};

/// Generate TypeScript bindings from all registered `#[remote]` procedures.
///
/// Writes three files to `config.output_dir`:
/// - `types.ts` — all input/output structs and enums
/// - `errors.ts` — `AppError<T>`, `TransportError`, `RpcResult`, procedure-specific error types
/// - `client.ts` — namespaced client functions calling the `rpc` helper
///
/// # Errors
///
/// Returns an error if the output directory cannot be created or files cannot be written.
pub fn generate(_config: Config) -> Result<(), GenerateError> {
    // Phase 1 implementation will:
    // 1. Collect procedures via inventory::iter
    // 2. Extract Specta type info
    // 3. Generate TS source via typescript, errors, and client modules
    // 4. Write files, skipping if content unchanged
    Ok(())
}

/// Errors that can occur during TypeScript generation.
#[derive(Debug)]
pub enum GenerateError {
    /// Failed to create the output directory.
    CreateDir(std::io::Error),
    /// Failed to write a generated file.
    WriteFile { path: std::path::PathBuf, source: std::io::Error },
    /// Specta type export failed.
    TypeExport(String),
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateDir(err) => write!(f, "failed to create output directory: {err}"),
            Self::WriteFile { path, source } => {
                write!(f, "failed to write {}: {source}", path.display())
            }
            Self::TypeExport(msg) => write!(f, "type export failed: {msg}"),
        }
    }
}

impl std::error::Error for GenerateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDir(err) | Self::WriteFile { source: err, .. } => Some(err),
            Self::TypeExport(_) => None,
        }
    }
}
