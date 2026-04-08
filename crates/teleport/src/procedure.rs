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

/// Metadata for a registered remote procedure. Populated by `#[remote]` via
/// `inventory::submit!` and collected at runtime by `TeleportRouter`.
pub struct ProcedureRegistration {
    /// Fully qualified name, e.g. `"users.getUser"`.
    pub name: &'static str,
    /// HTTP method derived from the procedure type.
    pub method: HttpMethod,
    /// Route path, e.g. `"/rpc/users.getUser"`.
    pub path: &'static str,
    /// Semantic procedure type.
    pub procedure_type: ProcedureType,
    /// Specta type info for the input parameter.
    pub input_type: fn() -> DataType,
    /// Specta type info for the output type.
    pub output_type: fn() -> DataType,
    /// Specta type info for the error detail type.
    pub error_type: fn() -> DataType,
    /// Doc comment from the Rust source.
    pub doc: &'static str,
}

inventory::collect!(ProcedureRegistration);
