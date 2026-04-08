use std::sync::Arc;

use axum::Router;

use crate::procedure::ProcedureRegistration;

/// Collects all `#[remote]` procedures and builds an Axum router.
pub struct TeleportRouter<S = ()> {
    _state: std::marker::PhantomData<S>,
}

impl<S> TeleportRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _state: std::marker::PhantomData,
        }
    }

    /// Set the application state shared across all procedures.
    #[must_use]
    pub fn state(self, _state: Arc<S>) -> Self {
        // Will store state for injection into handlers.
        self
    }

    /// Collect all registered procedures and build an Axum router.
    pub fn mount(self) -> Router<S> {
        let procedures: Vec<&ProcedureRegistration> =
            inventory::iter::<ProcedureRegistration>.into_iter().collect();

        let _ = procedures; // Will generate routes from these.

        Router::new()
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
