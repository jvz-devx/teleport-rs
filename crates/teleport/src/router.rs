use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;

use axum::response::Json;
use axum::routing::get;
use axum::Router;
use serde::Serialize;

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

    /// Collect all registered procedures and build an Axum router.
    ///
    /// Each `#[remote]` procedure's mount function is called with a type-erased
    /// router. The state type must match what the procedures were defined with.
    pub fn mount(self) -> Router {
        let Some(state) = self.state else {
            return Router::new();
        };

        let mut router: Box<dyn Any + Send> = Box::new(Router::<Arc<S>>::new());

        for reg in inventory::iter::<ProcedureRegistration> {
            let path = reg.path();
            match (reg.mount_fn)(router, &path) {
                Ok(updated) => router = updated,
                Err(original) => {
                    // State type mismatch — skip this registration.
                    // This shouldn't happen when all procedures use the same state.
                    router = original;
                }
            }
        }

        // Downcast back to the concrete router type and apply state.
        let mut final_router = router.downcast::<Router<Arc<S>>>().map_or_else(
            |_| Router::new(),
            |typed_router| typed_router.with_state(Arc::clone(&state)),
        );

        if self.manifest {
            let manifest = build_manifest();
            final_router =
                final_router.route("/rpc/__manifest", get(move || async move { Json(manifest) }));
        }

        final_router
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
