// Internal module for macro-generated code. Not part of the public API.
//
// The `#[remote]` proc macro generates `inventory::submit!` calls that
// reference types from this module. End users should never import from here.
// These re-exports are unused until the macro generates code that uses them.

#[allow(unused_imports)]
pub use crate::procedure::ProcedureRegistration;
#[allow(unused_imports)]
pub use inventory;
