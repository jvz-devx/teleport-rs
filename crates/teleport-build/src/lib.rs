pub mod client;
pub mod config;
pub mod errors;
pub mod naming;
pub(crate) mod ts_utils;
pub mod typescript;

pub use config::{Config, Naming, NamingCase, NamespaceStyle};

use std::path::Path;

use specta::ResolvedTypes;
use specta::datatype::DataType;

/// HTTP method for a remote procedure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

impl HttpMethod {
    /// Return the HTTP method string (`"GET"` or `"POST"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

/// Metadata for a single procedure, passed from the export binary to the
/// generator. This is a lightweight, owned mirror of `ProcedureRegistration`
/// that avoids a dependency on the `teleport` crate.
#[derive(Debug, Clone)]
pub struct ProcedureInfo {
    /// Fully qualified name, e.g. `"users.getUser"`.
    pub name: String,
    /// HTTP method derived from the procedure type.
    pub method: HttpMethod,
    /// Route path, e.g. `"/rpc/users.getUser"`.
    pub path: String,
    /// Doc comment from the Rust source.
    pub doc: String,
    /// Specta type info for the input parameter.
    pub input_type: DataType,
    /// Specta type info for the output type.
    pub output_type: DataType,
    /// Specta type info for the error detail type.
    pub error_type: DataType,
}

/// Generate TypeScript bindings from collected procedure metadata.
///
/// Writes three files to `config.output_dir`:
/// - `types.ts` — all input/output structs and enums
/// - `errors.ts` — `AppError<T>`, `TransportError`, `RpcResult`, procedure-specific error types
/// - `client.ts` — namespaced client functions calling the `rpc` helper
///
/// Files are only written when their content has actually changed, to avoid
/// unnecessary HMR triggers in the frontend dev server.
///
/// # Errors
///
/// Returns an error if the output directory cannot be created or files cannot be written.
pub fn generate(
    config: &Config,
    procedures: &[ProcedureInfo],
    resolved_types: &ResolvedTypes,
) -> Result<(), GenerateError> {
    std::fs::create_dir_all(&config.output_dir).map_err(GenerateError::CreateDir)?;

    let types_content = typescript::generate_types(resolved_types)?;
    let errors_content = errors::generate_errors(procedures, resolved_types)?;
    let client_content = client::generate_client(procedures, config, resolved_types)?;

    write_if_changed(&config.output_dir.join("types.ts"), &types_content)?;
    write_if_changed(&config.output_dir.join("errors.ts"), &errors_content)?;
    write_if_changed(&config.output_dir.join("client.ts"), &client_content)?;

    Ok(())
}

/// Write `content` to `path` only if the file does not already contain the
/// same bytes. This avoids touching the mtime and triggering unnecessary
/// Vite HMR cycles.
fn write_if_changed(path: &Path, content: &str) -> Result<(), GenerateError> {
    if let Ok(existing) = std::fs::read_to_string(path)
        && existing == content
    {
        return Ok(());
    }
    std::fs::write(path, content).map_err(|source| GenerateError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
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
