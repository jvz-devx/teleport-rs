use std::path::PathBuf;

/// Configuration for TypeScript generation.
pub struct Config {
    /// Directory to write generated `.ts` files into.
    pub output_dir: PathBuf,
    /// How procedure namespaces are derived.
    pub namespace_style: NamespaceStyle,
    /// Naming conventions for generated TypeScript identifiers.
    pub naming: Naming,
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
            route_prefix: "/rpc".to_owned(),
            client_import_path: None,
        }
    }

    /// Set a URL prefix for all RPC routes.
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.route_prefix = prefix.into();
        self
    }

    /// Override the import path for the `rpc` function.
    #[must_use]
    pub fn with_client_import(mut self, path: impl Into<String>) -> Self {
        self.client_import_path = Some(path.into());
        self
    }

    /// Set the namespace derivation style.
    #[must_use]
    pub const fn with_namespace_style(mut self, style: NamespaceStyle) -> Self {
        self.namespace_style = style;
        self
    }

    /// Set the naming convention for generated TypeScript identifiers.
    #[must_use]
    pub const fn with_naming(mut self, naming: Naming) -> Self {
        self.naming = naming;
        self
    }
}

/// Strategy for deriving procedure namespaces.
///
/// Currently only `ModulePath` is supported. This enum exists as an
/// extension point for future namespace strategies (e.g. flat, custom).
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
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
///
/// Currently only `CamelCase` is supported. This enum exists as an
/// extension point for future naming strategies (e.g. PascalCase, snake_case passthrough).
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub enum NamingCase {
    /// `snake_case` -> `camelCase` (TypeScript convention).
    #[default]
    CamelCase,
}
