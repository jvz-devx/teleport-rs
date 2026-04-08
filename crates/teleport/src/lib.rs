pub mod auth;
pub mod error;
pub mod extractors;
pub mod procedure;
pub mod router;

/// Internal module for macro-generated code. Not part of the public API.
pub mod private;

// Re-exports for user-facing API.
pub use auth::AuthConfig;
pub use error::AppError;
pub use extractors::{AuthedUser, QsQuery};
pub use procedure::{HttpMethod, ProcedureRegistration, ProcedureType};
pub use router::{Manifest, ManifestEntry, TeleportRouter};
pub use teleport_macros::{remote, teleport_type};
