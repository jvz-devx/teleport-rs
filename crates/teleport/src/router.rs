use std::any::Any;
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use axum::middleware;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use serde::Serialize;

use crate::auth::{auth_middleware, AuthConfig, AuthMiddlewareState};
use crate::extractors::AuthedUser;
use crate::procedure::{HttpMethod, ProcedureRegistration};

/// JSON payload returned by the `/__manifest` debug endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub procedures: BTreeMap<String, ManifestEntry>,
}

/// A single procedure entry in the manifest.
#[derive(Debug, Clone, Serialize)]
pub struct ManifestEntry {
    pub method: &'static str,
    pub path: String,
}

/// Collects all `#[remote]` procedures and builds an Axum router.
pub struct TeleportRouter<S = ()> {
    state: Option<Arc<S>>,
    manifest: bool,
    auth: Option<AuthConfig<S>>,
}

impl<S> TeleportRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: None,
            manifest: cfg!(feature = "debug-manifest"),
            auth: None,
        }
    }

    /// Set the application state shared across all procedures.
    #[must_use]
    pub fn state(mut self, state: Arc<S>) -> Self {
        self.state = Some(state);
        self
    }

    /// Enable or disable the `GET /rpc/__manifest` debug endpoint.
    ///
    /// By default, the manifest is mounted when the `debug-manifest` feature is
    /// enabled. Call this to override that behaviour explicitly.
    #[must_use]
    pub const fn manifest(mut self, enabled: bool) -> Self {
        self.manifest = enabled;
        self
    }

    /// Configure auth middleware that extracts session tokens from cookies
    /// or `Authorization: Bearer` headers and validates them into an
    /// [`AuthedUser`].
    ///
    /// The `validator` receives the extracted token and shared app state,
    /// returning `Some(AuthedUser)` if the token is valid. The middleware
    /// never blocks requests — procedure-level `AuthedUser` extractors
    /// handle 401 responses.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// TeleportRouter::new()
    ///     .state(Arc::new(app_state))
    ///     .auth("session", |token: String, state: Arc<AppState>| async move {
    ///         state.db.validate_session(&token).await.ok()
    ///     })
    ///     .mount()
    /// ```
    #[must_use]
    pub fn auth<F, Fut>(mut self, cookie_name: &str, validator: F) -> Self
    where
        F: Fn(String, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Option<AuthedUser>> + Send + 'static,
    {
        self.auth = Some(AuthConfig::new(cookie_name, validator));
        self
    }

    /// Collect all registered procedures and build an Axum router.
    ///
    /// Uses `inventory::iter` to discover all `#[remote]` procedures registered
    /// via `inventory::submit!`. Each procedure's type-erased mount function is
    /// called to add its route to the router.
    ///
    /// # Panics
    ///
    /// Panics if `.state()` was not called before `.mount()`.
    #[allow(clippy::expect_used, clippy::print_stderr)]
    pub fn mount(self) -> Router {
        let state = self.state.expect(
            "TeleportRouter::mount() called without .state() — call .state(Arc::new(your_state)) before .mount()",
        );

        let mut router: Box<dyn Any + Send> = Box::new(Router::<Arc<S>>::new());

        for reg in inventory::iter::<ProcedureRegistration> {
            let path = reg.path();
            match (reg.mount_fn)(router, &path) {
                Ok(updated) => router = updated,
                Err(original) => {
                    eprintln!(
                        "teleport-rs warning: state type mismatch for procedure '{}' — skipping",
                        reg.name()
                    );
                    router = original;
                }
            }
        }

        // Downcast back to the concrete router type and apply state.
        let mut final_router = router.downcast::<Router<Arc<S>>>().map_or_else(
            |_| Router::new(),
            |typed_router| {
                // Apply auth middleware before collapsing the state, so it
                // wraps all procedure routes.
                if let Some(auth) = self.auth {
                    let mw_state = Arc::new(AuthMiddlewareState {
                        auth,
                        app_state: Arc::clone(&state),
                    });
                    typed_router
                        .layer(middleware::from_fn_with_state(mw_state, auth_middleware))
                        .with_state(Arc::clone(&state))
                } else {
                    typed_router.with_state(Arc::clone(&state))
                }
            },
        );

        if self.manifest {
            let manifest = build_manifest();
            final_router =
                final_router.route("/rpc/__manifest", get(move || async move { Json(manifest) }));
        }

        final_router
    }
}

#[cfg(feature = "export")]
impl<S> TeleportRouter<S> {
    /// Generate TypeScript bindings from all registered procedures.
    /// Call this from your server binary during development.
    pub fn export(config: &teleport_build::Config) -> Result<(), teleport_build::GenerateError> {
        teleport_build::export_from_inventory(config)
    }
}

#[cfg(feature = "export")]
impl<S> TeleportRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Generate TypeScript bindings as part of the builder chain.
    /// Bindings are generated before the router is built.
    #[must_use]
    #[allow(clippy::print_stderr)]
    pub fn export_ts(self, config: &teleport_build::Config) -> Self {
        if let Err(e) = teleport_build::export_from_inventory(config) {
            eprintln!("teleport-rs: failed to export TypeScript bindings: {e}");
        }
        self
    }
}

impl<S> Default for TeleportRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Build a [`Manifest`] from all inventory-registered procedures.
fn build_manifest() -> Manifest {
    let mut procedures = BTreeMap::new();
    for reg in inventory::iter::<ProcedureRegistration> {
        let method = match reg.method {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
        };
        procedures.insert(
            reg.name(),
            ManifestEntry {
                method,
                path: reg.path(),
            },
        );
    }
    Manifest { procedures }
}
