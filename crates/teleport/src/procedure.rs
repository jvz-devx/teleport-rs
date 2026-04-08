use std::any::Any;

use specta::datatype::DataType;

/// HTTP method for a remote procedure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// Semantic procedure type, mirroring `SvelteKit` conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcedureType {
    /// Read-only data fetching (GET).
    Query,
    /// Mutations and actions (POST with JSON body).
    Command,
    /// Form submissions with progressive enhancement (POST).
    Form,
}

impl ProcedureType {
    #[must_use]
    pub const fn http_method(self) -> HttpMethod {
        match self {
            Self::Query => HttpMethod::Get,
            Self::Command | Self::Form => HttpMethod::Post,
        }
    }
}

/// Type-erased mount function signature.
///
/// Takes a boxed `Router<Arc<S>>` and a route path, returns the modified router.
/// Returns `Err` with the original router if the state type doesn't match (should
/// not happen in practice since all procedures in a binary share the same state).
pub type ErasedMountFn = fn(
    Box<dyn Any + Send>,
    &str,
) -> Result<Box<dyn Any + Send>, Box<dyn Any + Send>>;

/// Metadata for a registered remote procedure. Populated by `#[remote]` via
/// `inventory::submit!` and collected at runtime by `TeleportRouter`.
pub struct ProcedureRegistration {
    /// Raw module path from `module_path!()`, e.g. `"my_app::api::users"`.
    pub module_path: &'static str,
    /// Function name in `camelCase`.
    pub fn_name: &'static str,
    /// Optional prefix override (replaces module-derived namespace).
    pub prefix: Option<&'static str>,
    /// HTTP method derived from the procedure type.
    pub method: HttpMethod,
    /// Semantic procedure type.
    pub procedure_type: ProcedureType,
    /// Specta type info for the input parameter.
    ///
    /// Accepts a shared `Types` collection so the export binary can gather all
    /// type definitions into a single registry.
    pub input_type: fn(&mut specta::Types) -> DataType,
    /// Specta type info for the output type.
    pub output_type: fn(&mut specta::Types) -> DataType,
    /// Specta type info for the error detail type.
    pub error_type: fn(&mut specta::Types) -> DataType,
    /// Doc comment from the Rust source.
    pub doc: &'static str,
    /// Type-erased function that mounts this procedure's route.
    pub mount_fn: ErasedMountFn,
}

impl ProcedureRegistration {
    /// The namespace portion: either the prefix override or the last segment of the module path.
    #[must_use]
    pub fn namespace(&self) -> &str {
        if let Some(prefix) = self.prefix {
            return prefix;
        }
        self.module_path
            .rsplit("::")
            .next()
            .unwrap_or(self.module_path)
    }

    /// Fully qualified procedure name, e.g. `"users.getUser"`.
    #[must_use]
    pub fn name(&self) -> String {
        format!("{}.{}", self.namespace(), self.fn_name)
    }

    /// Route path, e.g. `"/rpc/users.getUser"`.
    #[must_use]
    pub fn path(&self) -> String {
        format!("/rpc/{}.{}", self.namespace(), self.fn_name)
    }
}

inventory::collect!(ProcedureRegistration);
