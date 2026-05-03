use serde::{Deserialize, Serialize};

/// Current version of the serialized contract schema.
pub const CONTRACT_VERSION: &str = "teleport.contract/v1";

/// Serialized contract emitted by backend implementations and consumed by code generators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractBundle {
    /// Version of the contract schema used by this bundle.
    pub version: String,
    /// Procedure descriptors exposed by the backend.
    pub procedures: Vec<ProcedureContract>,
    /// Named types referenced by procedures.
    pub types: Vec<NamedTypeContract>,
}

impl ContractBundle {
    /// Create an empty bundle with the current schema version.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: CONTRACT_VERSION.to_owned(),
            procedures: Vec::new(),
            types: Vec::new(),
        }
    }
}

impl Default for ContractBundle {
    fn default() -> Self {
        Self::new()
    }
}

/// Cross-language descriptor for a remotely callable procedure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcedureContract {
    pub name: String,
    pub namespace: String,
    pub method_name: String,
    pub procedure_kind: ProcedureKind,
    pub http_method: HttpMethod,
    pub path: String,
    pub input_encoding: InputEncoding,
    pub auth_mode: AuthMode,
    pub doc: String,
    pub input_type: TypeExpr,
    pub output_type: TypeExpr,
    pub error_type: TypeExpr,
}

/// How the procedure is intended to be used semantically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcedureKind {
    Query,
    Command,
    Form,
}

/// HTTP method exposed for the procedure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
}

impl HttpMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

/// Encoding used for the procedure input payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEncoding {
    None,
    QueryString,
    JsonBody,
    FormBody,
}

/// Auth requirement visible in the exported contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMode {
    None,
    Required,
    Optional,
}

/// Portable type expression used both in procedures and named type definitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    Primitive(PrimitiveType),
    List(Box<Self>),
    Map { key: Box<Self>, value: Box<Self> },
    Tuple(Vec<Self>),
    Nullable(Box<Self>),
    Named { name: String, generics: Vec<Self> },
    Generic(String),
    Opaque(String),
}

/// Portable primitive type set.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveType {
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    f16,
    f32,
    f64,
    f128,
    bool,
    char,
    str,
}

/// Named type referenced by procedures or other named types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedTypeContract {
    pub name: String,
    pub docs: String,
    pub generics: Vec<String>,
    pub kind: NamedTypeKind,
}

/// Portable representation of a named type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamedTypeKind {
    Struct(FieldsContract),
    Enum(Vec<VariantContract>),
    Alias(TypeExpr),
}

/// Fields used by structs and enum variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldsContract {
    Unit,
    Named(Vec<NamedFieldContract>),
    Unnamed(Vec<UnnamedFieldContract>),
}

/// Named field in a struct or enum variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedFieldContract {
    pub name: String,
    pub docs: String,
    pub optional: bool,
    pub ty: Option<TypeExpr>,
}

/// Unnamed field in a tuple struct or enum variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnnamedFieldContract {
    pub docs: String,
    pub ty: Option<TypeExpr>,
}

/// Enum variant definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantContract {
    pub name: String,
    pub docs: String,
    pub fields: FieldsContract,
}
