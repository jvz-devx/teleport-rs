use std::any::Any;
use std::sync::Arc;

use axum::Router;

use crate::procedure::ProcedureRegistration;

/// Collects all `#[remote]` procedures and builds an Axum router.
pub struct TeleportRouter<S = ()> {
    state: Option<Arc<S>>,
}

impl<S> TeleportRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    #[must_use]
    pub const fn new() -> Self {
        Self { state: None }
    }

    /// Set the application state shared across all procedures.
    #[must_use]
    pub fn state(mut self, state: Arc<S>) -> Self {
        self.state = Some(state);
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
        router.downcast::<Router<Arc<S>>>().map_or_else(
            |_| Router::new(),
            |typed_router| typed_router.with_state(Arc::clone(&state)),
        )
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
