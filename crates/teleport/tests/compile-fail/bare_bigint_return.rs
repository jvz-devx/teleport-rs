//! Bare 64-bit integer return types must be rejected at macro time —
//! JavaScript's `number` loses precision above 2^53 and teleport-rs
//! serialises 64-bit integers as JSON strings, which the `#[remote]`
//! macro can only wire up through struct fields, not bare primitives.

use teleport::{AppError, remote};

#[derive(Clone)]
struct State;

#[remote(query, prefix = "test")]
async fn bare_i64(_ctx: &State) -> Result<i64, AppError> {
    Ok(42)
}

fn main() {}
