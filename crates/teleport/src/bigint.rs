//! Serde helpers that serialise 64-bit integer primitives as JSON strings.
//!
//! These modules are used by `#[teleport_type]` to transparently rewrite
//! struct fields whose type is `i64` / `u64` / `i128` / `u128` / `isize` /
//! `usize` (or `Option<T>` of any of those) so that they round-trip as
//! JSON strings rather than JSON numbers.
//!
//! # Why
//!
//! JavaScript's `number` loses precision above 2^53 (≈ 9 × 10^15). Any
//! 64-bit integer value beyond that range silently rounds when parsed by
//! `JSON.parse`. To keep the Rust ↔ TypeScript round-trip bit-exact,
//! teleport-rs emits 64-bit integers as JSON strings on both sides:
//!
//! - The generated TypeScript type uses `string` (not `number`) for
//!   64-bit fields. That rewrite lives in
//!   `teleport-build::typescript::rewrite_bigint_to_string`.
//! - The `#[teleport_type]` proc macro injects
//!   `#[serde(with = "::teleport::private::bigint::i64_as_string")]`
//!   (or the appropriate variant) onto every 64-bit field so the Rust
//!   side serialises the value as a JSON string. That rewrite lives in
//!   `teleport-macros::teleport_type::inject_bigint_attrs`.
//!
//! Both rewrites are required; the TypeScript-only rewrite produced a
//! type lie (`string` in TS, `number` on the wire), and the Rust-only
//! rewrite would panic at export time because specta-typescript refuses
//! 64-bit primitives outright.
//!
//! Users should never import from this module directly — it exists only
//! to give `#[serde(with = ...)]` a stable path to reference.

#![allow(missing_docs)]
// internal helpers
// `&Option<T>` is forced on us by the serde `Serialize::serialize(&self, …)`
// contract; we can't drop the reference or the trait impl won't satisfy
// `#[serde(with = "…")]`. The clippy lint flags it as non-idiomatic which is
// correct in user code but wrong for serde-helper modules specifically.
#![allow(clippy::ref_option)]

use std::fmt::Display;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serializer};

/// Serialize any `Display` integer as a JSON string.
fn ser_as_string<S: Serializer, T: Display>(v: &T, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&v.to_string())
}

/// Deserialize from a JSON string into any `FromStr` integer.
fn de_from_string<'de, D: Deserializer<'de>, T: FromStr>(d: D) -> Result<T, D::Error>
where
    T::Err: Display,
{
    let s = String::deserialize(d)?;
    s.parse().map_err(serde::de::Error::custom)
}

/// Serialize `Option<T: Display>` as either `null` or a JSON string.
fn ser_opt_as_string<S: Serializer, T: Display>(v: &Option<T>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(n) => s.serialize_some(&n.to_string()),
        None => s.serialize_none(),
    }
}

/// Deserialize `Option<T: FromStr>` from either `null` or a JSON string.
fn de_opt_from_string<'de, D: Deserializer<'de>, T: FromStr>(d: D) -> Result<Option<T>, D::Error>
where
    T::Err: Display,
{
    let opt: Option<String> = Option::deserialize(d)?;
    opt.map(|s| s.parse().map_err(serde::de::Error::custom))
        .transpose()
}

// Macro-generated helper modules. Each produces a `mod <name> { serialize, deserialize }`
// pair that can be passed to `#[serde(with = "...")]`.

macro_rules! as_string_module {
    ($name:ident, $ty:ty) => {
        pub mod $name {
            use serde::{Deserializer, Serializer};
            #[allow(clippy::trivially_copy_pass_by_ref)]
            pub fn serialize<S: Serializer>(v: &$ty, s: S) -> Result<S::Ok, S::Error> {
                super::ser_as_string(v, s)
            }
            pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<$ty, D::Error> {
                super::de_from_string::<D, $ty>(d)
            }
        }
    };
}

macro_rules! opt_as_string_module {
    ($name:ident, $ty:ty) => {
        pub mod $name {
            use serde::{Deserializer, Serializer};
            pub fn serialize<S: Serializer>(v: &Option<$ty>, s: S) -> Result<S::Ok, S::Error> {
                super::ser_opt_as_string(v, s)
            }
            pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<$ty>, D::Error> {
                super::de_opt_from_string::<D, $ty>(d)
            }
        }
    };
}

as_string_module!(i64_as_string, i64);
as_string_module!(u64_as_string, u64);
as_string_module!(i128_as_string, i128);
as_string_module!(u128_as_string, u128);
as_string_module!(isize_as_string, isize);
as_string_module!(usize_as_string, usize);

opt_as_string_module!(opt_i64_as_string, i64);
opt_as_string_module!(opt_u64_as_string, u64);
opt_as_string_module!(opt_i128_as_string, i128);
opt_as_string_module!(opt_u128_as_string, u128);
opt_as_string_module!(opt_isize_as_string, isize);
opt_as_string_module!(opt_usize_as_string, usize);
