use std::path::PathBuf;

/// Configuration for TypeScript generation.
pub struct Config {
    /// Directory to write generated `.ts` files into.
    pub output_dir: PathBuf,
    /// How procedure namespaces are derived.
    pub namespace_style: NamespaceStyle,
    /// Naming conventions for generated TypeScript identifiers.
    pub naming: Naming,
    /// Whether to include the debug manifest endpoint.
    pub include_manifest: bool,
    /// URL prefix for all RPC routes (e.g. `"/rpc"`).
    pub route_prefix: String,
    /// Override the import path for the `rpc` function.
    /// Defaults to `"@teleport-rs/client"` when `None`.
    pub client_import_path: Option<String>,
}

impl Config {
    /// Create a new configuration with the given output directory and sensible defaults.
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
            namespace_style: NamespaceStyle::default(),
            naming: Naming::default(),
            include_manifest: false,
            route_prefix: "/rpc".to_owned(),
            client_import_path: None,
        }
    }
}

/// Strategy for deriving procedure namespaces.
#[derive(Debug, Clone, Copy, Default)]
pub enum NamespaceStyle {
    /// Derive namespace from the Rust module path (e.g. `api::users` -> `users`).
    #[default]
    ModulePath,
}

/// Naming conventions for generated TypeScript code.
pub struct Naming {
    /// Case convention for function and property names.
    pub case: NamingCase,
}

impl Default for Naming {
    fn default() -> Self {
        Self {
            case: NamingCase::CamelCase,
        }
    }
}

/// Case convention for TypeScript identifiers.
#[derive(Debug, Clone, Copy, Default)]
pub enum NamingCase {
    /// `snake_case` -> `camelCase` (TypeScript convention).
    #[default]
    CamelCase,
}
