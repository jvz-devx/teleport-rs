#![warn(missing_docs)]
//! teleport-rs — write Rust, call from TypeScript with full type safety.
//!
//! Annotate functions with `#[remote]` and structs with `#[teleport_type]`,
//! then build an Axum router with [`TeleportRouter`]. The same binary can
//! emit a fully typed TypeScript client via the `export` feature.
//!
//! See the [README](https://github.com/jvz-devx/teleport-rs) and the
//! [getting started guide](https://github.com/jvz-devx/teleport-rs/blob/main/docs/getting-started.md)
//! for a walkthrough. For the production checklist, see
//! [`docs/security.md`](https://github.com/jvz-devx/teleport-rs/blob/main/docs/security.md).

/// Auth middleware configuration and helpers.
pub mod auth;
#[doc(hidden)]
pub mod bigint;
/// Framework error type [`AppError`] and its HTTP response mapping.
pub mod error;
/// Axum extractors used by generated procedure handlers.
pub mod extractors;
/// Type-erased procedure registration collected via `inventory`.
pub mod procedure;
/// Router builder that turns registered procedures into an Axum `Router`.
pub mod router;

/// Internal module for macro-generated code. Not part of the public API.
pub mod private;

// Re-exports for user-facing API.
pub use auth::AuthConfig;
pub use error::AppError;
pub use extractors::{AuthedUser, FormOrJson, QsQuery, TeleportUser};
pub use procedure::{HttpMethod, ProcedureRegistration, ProcedureType};
pub use router::{Manifest, ManifestEntry, NoState, TeleportRouter, WithState};
pub use teleport_macros::{remote, teleport_type};

#[cfg(feature = "export")]
pub use teleport_build::Config as ExportConfig;
#[cfg(feature = "export")]
pub use teleport_build::ContractBundle;
