use std::collections::BTreeMap;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use axum::middleware;
use axum::response::Json;
use axum::routing::{get, MethodRouter};
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

/// Marker: state has not been provided yet.
pub struct NoState;

/// Marker: state has been provided via `.state()`.
pub struct WithState;

/// Callback type for the per-route hook set via [`TeleportRouter::on_route`].
///
/// Receives the route path (e.g. `"/rpc/users.getUser"`) and the
/// [`MethodRouter`] for that procedure. Return a (possibly wrapped)
/// `MethodRouter` — this is where you add per-procedure middleware.
type RouteHook<S> =
    dyn Fn(&str, MethodRouter<Arc<S>>) -> MethodRouter<Arc<S>> + Send + Sync;

/// Collects all `#[remote]` procedures and builds an Axum router.
///
/// Uses a typestate pattern: `.mount()` and `.auth()` are only available
/// after `.state()` has been called, turning `TeleportRouter<S, NoState>`
/// into `TeleportRouter<S, WithState>`.
pub struct TeleportRouter<S = (), StateMarker = NoState> {
    state: Option<Arc<S>>,
    manifest: bool,
    auth: Option<AuthConfig<S>>,
    route_hook: Option<Arc<RouteHook<S>>>,
    _marker: PhantomData<StateMarker>,
}

impl<S> TeleportRouter<S, NoState>
where
    S: Clone + Send + Sync + 'static,
{
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: None,
            manifest: cfg!(feature = "debug-manifest"),
            auth: None,
            route_hook: None,
            _marker: PhantomData,
        }
    }

    /// Set the application state shared across all procedures.
    ///
    /// This transitions the router from `NoState` to `WithState`, enabling
    /// `.mount()` and `.auth()`.
    #[must_use]
    pub fn state(self, state: Arc<S>) -> TeleportRouter<S, WithState> {
        TeleportRouter {
            state: Some(state),
            manifest: self.manifest,
            auth: self.auth,
            route_hook: self.route_hook,
            _marker: PhantomData,
        }
    }
}

impl<S> TeleportRouter<S, WithState>
where
    S: Clone + Send + Sync + 'static,
{
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

    /// Register a callback that can inspect or wrap each procedure's
    /// [`MethodRouter`] before it is added to the final Axum router.
    ///
    /// The callback receives the route path (e.g. `"/rpc/admin.deleteUser"`)
    /// and the `MethodRouter` for that procedure. Return a (possibly wrapped)
    /// `MethodRouter` — this is where you add per-procedure middleware.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use axum::middleware;
    ///
    /// TeleportRouter::new()
    ///     .state(state)
    ///     .on_route(|path, route| {
    ///         if path.starts_with("/rpc/admin.") {
    ///             route.layer(RateLimitLayer::new(10))
    ///         } else {
    ///             route
    ///         }
    ///     })
    ///     .mount()
    /// ```
    #[must_use]
    pub fn on_route<F>(mut self, hook: F) -> Self
    where
        F: Fn(&str, MethodRouter<Arc<S>>) -> MethodRouter<Arc<S>> + Send + Sync + 'static,
    {
        self.route_hook = Some(Arc::new(hook));
        self
    }

    /// Collect all registered procedures and build an Axum router.
    ///
    /// Uses `inventory::iter` to discover all `#[remote]` procedures registered
    /// via `inventory::submit!`. Each procedure's type-erased mount function is
    /// called to create a `MethodRouter`, which is then (optionally) passed
    /// through the [`on_route`](Self::on_route) hook before being added to
    /// the router.
    #[allow(clippy::print_stderr)]
    pub fn mount(self) -> Router {
        // SAFETY: state is guaranteed to be `Some` by the typestate — only
        // `WithState` has this method, and transitioning to `WithState`
        // always sets `self.state = Some(...)`.
        let state = self.state.unwrap_or_else(|| unreachable!());

        let mut router = Router::<Arc<S>>::new();

        for reg in inventory::iter::<ProcedureRegistration> {
            let path = reg.path();
            let method_router_any = (reg.mount_fn)();

            let Some(method_router) =
                method_router_any.downcast::<MethodRouter<Arc<S>>>().ok()
            else {
                eprintln!(
                    "teleport-rs warning: state type mismatch for procedure '{}' — skipping",
                    reg.name()
                );
                continue;
            };

            let method_router = match self.route_hook {
                Some(ref hook) => hook(&path, *method_router),
                None => *method_router,
            };

            router = router.route(&path, method_router);
        }

        let mut final_router = if let Some(auth) = self.auth {
            let mw_state = Arc::new(AuthMiddlewareState {
                auth,
                app_state: Arc::clone(&state),
            });
            router
                .layer(middleware::from_fn_with_state(mw_state, auth_middleware))
                .with_state(Arc::clone(&state))
        } else {
            router.with_state(Arc::clone(&state))
        };

        if self.manifest {
            let manifest = build_manifest();
            final_router =
                final_router.route("/rpc/__manifest", get(move || async move { Json(manifest) }));
        }

        final_router
    }
}

/// Methods available regardless of state.
impl<S, St> TeleportRouter<S, St>
where
    S: Clone + Send + Sync + 'static,
{
    /// Enable or disable the `GET /rpc/__manifest` debug endpoint.
    ///
    /// By default, the manifest is mounted when the `debug-manifest` feature is
    /// enabled. Call this to override that behaviour explicitly.
    #[must_use]
    pub const fn manifest(mut self, enabled: bool) -> Self {
        self.manifest = enabled;
        self
    }
}

#[cfg(feature = "export")]
impl<S, St> TeleportRouter<S, St> {
    /// Generate TypeScript bindings from all registered procedures.
    /// Call this from your server binary during development.
    pub fn export(config: &teleport_build::Config) -> Result<(), teleport_build::GenerateError> {
        teleport_build::export_from_inventory(config)
    }
}

#[cfg(feature = "export")]
impl<S, St> TeleportRouter<S, St>
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

impl<S> Default for TeleportRouter<S, NoState>
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
