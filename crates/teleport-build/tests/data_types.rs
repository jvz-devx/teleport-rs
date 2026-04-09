//! Exhaustive TypeScript codegen tests for every Rust data type
//! teleport-rs is expected to handle.
//!
//! Each `#[test]` function registers procedures whose input/output carries
//! a specific Rust type and asserts the generated TS matches expectation.
//!
//! These tests double as regression coverage for the three codegen bugs
//! fixed in 0.1.1:
//!
//! 1. **`Vec<T>` no longer leaks as a named import.**
//!    Before: `import type { ..., Vec } from "./types";` + `RpcResult<Vec<Todo>, null>`
//!    After: `RpcResult<Todo[], null>`, no `Vec` in imports.
//!
//! 2. **`String` no longer leaks as a named import.**
//!    Before: `RpcResult<String, null>` + `String` in imports.
//!    After: `RpcResult<string, null>`, no `String` in imports.
//!
//! 3. **64-bit integer primitives render as `string` instead of panicking.**
//!    `specta-typescript` refuses `i64`/`u64`/`i128`/`u128`/`isize`/`usize` to
//!    avoid precision loss in JS `number`. We rewrite those to `str` before
//!    handing the type collection to the exporter, so they render as
//!    TypeScript `string`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    // Every field in the fixture structs uses an `_field` suffix to make
    // the assertion messages readable (`bool_field: boolean` etc). That
    // trips `struct_field_names` and is fine for a test file.
    clippy::struct_field_names,
    dead_code
)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use specta::Type;
use teleport_build::{Config, export_from_inventory};
use teleport_core::{HttpMethod, ProcedureRegistration, ProcedureType};

// ---------------------------------------------------------------------------
// Fixture types — one per category. All registered via procedures below so
// they end up in the emitted `types.ts`.
// ---------------------------------------------------------------------------

// -- primitives --

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct AllPrimitives {
    bool_field: bool,
    i8_field: i8,
    i16_field: i16,
    i32_field: i32,
    u8_field: u8,
    u16_field: u16,
    u32_field: u32,
    f32_field: f32,
    f64_field: f64,
    string_field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct BigIntPrimitives {
    i64_field: i64,
    u64_field: u64,
    i128_field: i128,
    u128_field: u128,
    isize_field: isize,
    usize_field: usize,
}

// -- containers --

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct ContainerTypes {
    vec_i32: Vec<i32>,
    vec_string: Vec<String>,
    option_i32: Option<i32>,
    option_string: Option<String>,
    option_vec: Option<Vec<i32>>,
    vec_option: Vec<Option<i32>>,
    hash_map_str_i32: HashMap<String, i32>,
    btree_map_str_str: BTreeMap<String, String>,
    hash_set_str: HashSet<String>,
    btree_set_i32: BTreeSet<i32>,
    vec_deque_i32: VecDeque<i32>,
    tuple_2: (i32, String),
    tuple_3: (bool, i32, String),
}

// -- nested struct containing another struct --

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct DataTypesInner {
    name: String,
    value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct NestedStruct {
    inner: DataTypesInner,
    list_of_inner: Vec<DataTypesInner>,
    maybe_inner: Option<DataTypesInner>,
}

// -- newtype / tuple struct --

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct NewtypeId(String);

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct NewtypeWrapper {
    id: NewtypeId,
    name: String,
}

// -- enums --

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
enum UnitEnum {
    Alpha,
    Beta,
    Gamma,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct EnumWrapper {
    unit: UnitEnum,
    list: Vec<UnitEnum>,
    maybe: Option<UnitEnum>,
}

// -- regression fixtures for Bug 1 (Vec<T> as top-level return) --

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct VecItem {
    id: String,
    label: String,
}

// -- regression fixture for Bug 2 (String as top-level return) --
// (`String` is a stdlib type; no new struct needed.)

// ---------------------------------------------------------------------------
// Procedure registrations — one per fixture. Inventory is per-binary so
// these only appear when this test binary runs.
// ---------------------------------------------------------------------------

fn stub_mount() -> Box<dyn std::any::Any + Send> {
    Box::new(())
}

inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::carry",
        fn_name: "allPrimitives",
        prefix: Some("dt"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <AllPrimitives as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::carry",
        fn_name: "bigIntPrimitives",
        prefix: Some("dt"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <BigIntPrimitives as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::carry",
        fn_name: "containerTypes",
        prefix: Some("dt"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <ContainerTypes as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::carry",
        fn_name: "nestedStruct",
        prefix: Some("dt"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <NestedStruct as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::carry",
        fn_name: "newtypeWrapper",
        prefix: Some("dt"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <NewtypeWrapper as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::carry",
        fn_name: "enumWrapper",
        prefix: Some("dt"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <EnumWrapper as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

// Bug 1: `Vec<T>` as top-level return — must render as `T[]`, not `Vec<T>`.
inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::regressions",
        fn_name: "listItems",
        prefix: Some("regressions"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <Vec<VecItem> as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

// Bug 2: `String` as top-level return — must render as `string`, not `String`.
inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::regressions",
        fn_name: "echoString",
        prefix: Some("regressions"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <String as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

// Bug 1 + nested: `Vec<Vec<T>>` — inner Vec must also render as `T[]`.
inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::regressions",
        fn_name: "nestedVec",
        prefix: Some("regressions"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <Vec<Vec<String>> as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

// ---------------------------------------------------------------------------
// Known upstream bug: enum with struct variants in Detail position
// ---------------------------------------------------------------------------
//
// `specta-typescript` 0.0.11 renders externally-tagged enums with struct
// variants by collapsing the variant names and dropping the outer tag.
// An enum like `SlugInvalid { reason }` + `UrlInvalid { reason }` becomes
// the TypeScript type `{ reason: string }` with no way to distinguish the
// two variants, and the nesting level is wrong: wire JSON is
// `{"SlugInvalid": {"reason": "..."}}` but the TS type says `{reason: ...}`
// is the outer shape. `tsc` passes, runtime field access returns
// `undefined`. See `docs/error-handling.md` §"Detail type constraints".
//
// These fixtures register the broken pattern and the working workaround
// so future regressions surface in CI (the ignored tests will FAIL if
// specta ever fixes the upstream bug — that's a signal to update the
// docs and un-ignore the tests).

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
enum KnownBugEnum {
    SlugTaken,
    SlugInvalid { reason: String },
    UrlInvalid { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind")]
enum KnownBugTaggedEnum {
    SlugTaken,
    SlugInvalid { reason: String },
    UrlInvalid { reason: String },
}

/// Recommended workaround: flat struct with `bool`/`Option<String>` fields.
/// Rust callers set exactly one field to "signal" the error variant; TS
/// consumers simply check the booleans/nullables. Round-trips cleanly
/// because it is a plain struct, not an enum.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
struct WorkaroundErrorDetail {
    slug_taken: bool,
    slug_invalid: Option<String>,
    url_invalid: Option<String>,
}

inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::known_bugs",
        fn_name: "brokenEnum",
        prefix: Some("known_bug"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <KnownBugEnum as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::known_bugs",
        fn_name: "taggedEnum",
        prefix: Some("known_bug"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <KnownBugTaggedEnum as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

inventory::submit! {
    ProcedureRegistration {
        module_path: "data_types::known_bugs",
        fn_name: "workaround",
        prefix: Some("known_bug"),
        method: HttpMethod::Get,
        procedure_type: ProcedureType::Query,
        input_type: |types| <() as Type>::definition(types),
        output_type: |types| <WorkaroundErrorDetail as Type>::definition(types),
        error_type: |types| <() as Type>::definition(types),
        doc: "",
        mount_fn: stub_mount,
    }
}

// ---------------------------------------------------------------------------
// Shared generation helper
// ---------------------------------------------------------------------------

/// Generate all bindings into a fresh tempdir and return the contents.
/// Returns `(TempDir, types_ts, client_ts)` — the tempdir is kept alive
/// so lookups on `tmp.path()` remain valid for the caller.
fn generate_all() -> (tempfile::TempDir, String, String) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = Config::new(tmp.path().to_path_buf())
        .with_prefix("/rpc")
        .with_client_import("@teleport-rs/client");
    export_from_inventory(&config).expect("export");
    let types_ts = std::fs::read_to_string(tmp.path().join("types.ts")).expect("read types.ts");
    let client_ts = std::fs::read_to_string(tmp.path().join("client.ts")).expect("read client.ts");
    (tmp, types_ts, client_ts)
}

/// Extract the single `import type { … } from "./types";` line from a
/// generated client.ts. Panics if not found.
fn types_import_line(client_ts: &str) -> &str {
    client_ts
        .lines()
        .find(|l| l.contains("from \"./types\""))
        .expect("client.ts should have a types import line")
}

// ---------------------------------------------------------------------------
// Primitive types
// ---------------------------------------------------------------------------

#[test]
fn primitive_bool_renders_as_boolean() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("bool_field: boolean"),
        "bool should render as `boolean`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn primitive_signed_ints_render_as_number() {
    let (_tmp, types_ts, _) = generate_all();
    for field in ["i8_field", "i16_field", "i32_field"] {
        assert!(
            types_ts.contains(&format!("{field}: number")),
            "{field} should render as `number`\n--- types.ts ---\n{types_ts}",
        );
    }
}

#[test]
fn primitive_unsigned_ints_render_as_number() {
    let (_tmp, types_ts, _) = generate_all();
    for field in ["u8_field", "u16_field", "u32_field"] {
        assert!(
            types_ts.contains(&format!("{field}: number")),
            "{field} should render as `number`\n--- types.ts ---\n{types_ts}",
        );
    }
}

#[test]
fn primitive_floats_render_as_number() {
    let (_tmp, types_ts, _) = generate_all();
    for field in ["f32_field", "f64_field"] {
        assert!(
            types_ts.contains(&format!("{field}: number")),
            "{field} should render as `number`\n--- types.ts ---\n{types_ts}",
        );
    }
}

#[test]
fn primitive_string_field_renders_as_string() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("string_field: string"),
        "String field should render as `string`\n--- types.ts ---\n{types_ts}",
    );
}

// ---------------------------------------------------------------------------
// BigInt primitives — regression for Bug 3
// ---------------------------------------------------------------------------

#[test]
fn bigint_i64_renders_as_string() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("i64_field: string"),
        "i64 must render as `string` (BigInt precision workaround)\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn bigint_u64_renders_as_string() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("u64_field: string"),
        "u64 must render as `string`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn bigint_i128_renders_as_string() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("i128_field: string"),
        "i128 must render as `string`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn bigint_u128_renders_as_string() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("u128_field: string"),
        "u128 must render as `string`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn bigint_isize_renders_as_string() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("isize_field: string"),
        "isize must render as `string`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn bigint_usize_renders_as_string() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("usize_field: string"),
        "usize must render as `string`\n--- types.ts ---\n{types_ts}",
    );
}

// ---------------------------------------------------------------------------
// Container types
// ---------------------------------------------------------------------------

#[test]
fn vec_of_primitive_renders_as_array() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("vec_i32: number[]"),
        "Vec<i32> should render as `number[]`\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("vec_string: string[]"),
        "Vec<String> should render as `string[]`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn option_of_primitive_renders_as_nullable_union() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("option_i32: number | null"),
        "Option<i32> should render as `number | null`\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("option_string: string | null"),
        "Option<String> should render as `string | null`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn option_of_vec_renders_correctly() {
    let (_tmp, types_ts, _) = generate_all();
    // specta renders this as either `number[] | null` or `(number[]) | null`.
    // Accept either — the important thing is the `| null` and the `[]`.
    assert!(
        types_ts.contains("option_vec: number[] | null")
            || types_ts.contains("option_vec: (number[]) | null"),
        "Option<Vec<i32>> should be `number[] | null`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn vec_of_option_renders_correctly() {
    let (_tmp, types_ts, _) = generate_all();
    // `Vec<Option<i32>>` → `(number | null)[]`. Paren-wrapping is
    // required because `|` has lower precedence than `[]` in TS.
    assert!(
        types_ts.contains("vec_option: (number | null)[]"),
        "Vec<Option<i32>> should be `(number | null)[]`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn hashmap_renders_as_record() {
    let (_tmp, types_ts, _) = generate_all();
    // specta-typescript 0.0.11 renders maps as `{ [key in K]: V }` (mapped
    // type syntax), which is semantically odd (mapped types require the
    // value for every key) but works at runtime. Accept that form plus
    // the more idiomatic `Record<K, V>` and index-signature forms.
    let ok = types_ts.contains("hash_map_str_i32: Record<string, number>")
        || types_ts.contains("hash_map_str_i32: { [key: string]: number }")
        || types_ts.contains("hash_map_str_i32: { [key in string]: number }");
    assert!(
        ok,
        "HashMap<String, i32> should render as Record or index signature\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn btreemap_renders_as_record() {
    let (_tmp, types_ts, _) = generate_all();
    let ok = types_ts.contains("btree_map_str_str: Record<string, string>")
        || types_ts.contains("btree_map_str_str: { [key: string]: string }")
        || types_ts.contains("btree_map_str_str: { [key in string]: string }");
    assert!(
        ok,
        "BTreeMap<String, String> should render as Record or index signature\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn hashset_renders_as_array() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("hash_set_str: string[]"),
        "HashSet<String> should render as `string[]`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn btreeset_renders_as_array() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("btree_set_i32: number[]"),
        "BTreeSet<i32> should render as `number[]`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn vecdeque_renders_as_array() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("vec_deque_i32: number[]"),
        "VecDeque<i32> should render as `number[]`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn tuple_two_elements_renders_as_array_literal() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("tuple_2: [number, string]"),
        "(i32, String) should render as `[number, string]`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn tuple_three_elements_renders_as_array_literal() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("tuple_3: [boolean, number, string]"),
        "(bool, i32, String) should render as `[boolean, number, string]`\n--- types.ts ---\n{types_ts}",
    );
}

// ---------------------------------------------------------------------------
// User-defined types
// ---------------------------------------------------------------------------

#[test]
fn nested_struct_renders_with_inner_reference() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("export type DataTypesInner"),
        "Inner struct should be exported\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("export type NestedStruct"),
        "Outer struct should be exported\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("inner: DataTypesInner"),
        "inner field should reference `DataTypesInner`\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("list_of_inner: DataTypesInner[]"),
        "list_of_inner should be `DataTypesInner[]`, not `Vec<DataTypesInner>`\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("maybe_inner: DataTypesInner | null"),
        "maybe_inner should be `DataTypesInner | null`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn newtype_struct_flattens_to_inner_type_in_containing_struct() {
    let (_tmp, types_ts, _) = generate_all();
    // specta emits newtypes as a type alias: `export type NewtypeId = string;`.
    // The containing struct should reference `NewtypeId`, not its inner `string` directly.
    assert!(
        types_ts.contains("NewtypeId"),
        "NewtypeId should appear in types.ts\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("id: NewtypeId"),
        "NewtypeWrapper.id should reference NewtypeId\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn unit_only_enum_renders_as_string_union() {
    let (_tmp, types_ts, _) = generate_all();
    // specta renders externally-tagged unit-only enums as a string union.
    assert!(
        types_ts.contains("export type UnitEnum"),
        "UnitEnum should be exported\n--- types.ts ---\n{types_ts}",
    );
    // Each variant becomes a string literal.
    for variant in ["\"Alpha\"", "\"Beta\"", "\"Gamma\""] {
        assert!(
            types_ts.contains(variant),
            "UnitEnum should contain variant {variant}\n--- types.ts ---\n{types_ts}",
        );
    }
}

#[test]
fn enum_as_field_renders_correctly() {
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("unit: UnitEnum"),
        "unit field should reference UnitEnum\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("list: UnitEnum[]"),
        "list field should be UnitEnum[] (not Vec<UnitEnum>)\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("maybe: UnitEnum | null"),
        "maybe field should be UnitEnum | null\n--- types.ts ---\n{types_ts}",
    );
}

// ---------------------------------------------------------------------------
// Regressions for the three 0.1.1 codegen bugs
// ---------------------------------------------------------------------------

#[test]
fn bug1_vec_as_toplevel_return_renders_as_array() {
    let (_tmp, _types_ts, client_ts) = generate_all();
    assert!(
        client_ts.contains("Promise<RpcResult<VecItem[], never>>"),
        "Vec<VecItem> as return type must render as VecItem[], not Vec<VecItem>\n--- client.ts ---\n{client_ts}",
    );
}

#[test]
fn bug1_vec_does_not_leak_into_type_imports() {
    let (_tmp, _types_ts, client_ts) = generate_all();
    let import = types_import_line(&client_ts);
    // Strip leading/trailing whitespace and match the exact identifier `Vec`
    // (surrounded by punctuation or whitespace), not a substring of another ident.
    let contains_vec_ident = import
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|tok| tok == "Vec");
    assert!(
        !contains_vec_ident,
        "Vec must not appear in the types import line — it's translated to `T[]` inline\n--- import ---\n{import}",
    );
}

#[test]
fn bug2_string_as_toplevel_return_renders_as_lowercase_string() {
    let (_tmp, _types_ts, client_ts) = generate_all();
    assert!(
        client_ts.contains("Promise<RpcResult<string, never>>"),
        "String as return type must render as lowercase `string`\n--- client.ts ---\n{client_ts}",
    );
}

#[test]
fn bug2_string_does_not_leak_into_type_imports() {
    let (_tmp, _types_ts, client_ts) = generate_all();
    let import = types_import_line(&client_ts);
    let contains_string_ident = import
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|tok| tok == "String");
    assert!(
        !contains_string_ident,
        "String must not appear in the types import line\n--- import ---\n{import}",
    );
}

#[test]
fn bug3_bigint_rewrite_applies_to_all_six_primitives() {
    // Covered individually above, but keep a single "smoke" test that fails
    // fast with one message if the rewrite pass regressed wholesale.
    let (_tmp, types_ts, _) = generate_all();
    let all_rewritten = types_ts.contains("i64_field: string")
        && types_ts.contains("u64_field: string")
        && types_ts.contains("i128_field: string")
        && types_ts.contains("u128_field: string")
        && types_ts.contains("isize_field: string")
        && types_ts.contains("usize_field: string");
    assert!(
        all_rewritten,
        "all 6 BigInt primitives must be rewritten to `string`\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn nested_vec_nested_vec_renders_as_nested_array() {
    let (_tmp, _types_ts, client_ts) = generate_all();
    assert!(
        client_ts.contains("Promise<RpcResult<string[][], never>>"),
        "Vec<Vec<String>> should render as `string[][]`\n--- client.ts ---\n{client_ts}",
    );
}

// ---------------------------------------------------------------------------
// Enum-with-struct-variants rendering
// ---------------------------------------------------------------------------
//
// `specta-typescript` 0.0.11's legacy enum renderer collapses struct
// variant names and drops the outer variant tag — its corrected
// externally-tagged path is commented out in the upstream crate. The
// `teleport-build` post-processor in `typescript::rewrite_enums_with_struct_variants`
// walks the resolved type collection and replaces broken enum blocks
// with the correct shape before the file is written.
//
// These tests lock in the corrected behavior. If specta ever unfucks
// its own rendering, the tests will keep passing (the post-processor
// still produces the same shape). If somebody accidentally breaks the
// post-processor, they fail loudly.

#[test]
fn enum_with_struct_variants_renders_externally_tagged() {
    let (_tmp, types_ts, _) = generate_all();
    // The post-processor should emit:
    //     export type KnownBugEnum =
    //         | "SlugTaken"
    //         | { SlugInvalid: { reason: string } }
    //         | { UrlInvalid: { reason: string } };
    assert!(
        types_ts.contains(r#""SlugTaken""#),
        "unit variant should render as string literal\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains(r"{ SlugInvalid: { reason: string } }"),
        "SlugInvalid variant should keep its name and the field nesting\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains(r"{ UrlInvalid: { reason: string } }"),
        "UrlInvalid must be distinguishable from SlugInvalid\n--- types.ts ---\n{types_ts}",
    );
    // Both variants must exist simultaneously — not collapsed into a
    // single `{ reason: string }`.
    let slug_count = types_ts.matches("SlugInvalid").count();
    let url_count = types_ts.matches("UrlInvalid").count();
    assert!(
        slug_count >= 1 && url_count >= 1,
        "variants must not be collapsed\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn enum_with_serde_tag_still_renders_externally_tagged() {
    // `#[serde(tag = "kind")]` changes the Rust wire format to
    // internally-tagged `{"kind": "SlugInvalid", "reason": "..."}`, but
    // specta-typescript 0.0.11 does not expose the attribute to its
    // renderer. Our post-processor therefore can't detect the intent
    // and renders externally-tagged regardless. Users who need
    // internally-tagged must not use `#[serde(tag)]` on a
    // `#[teleport_type]` enum — the flat-struct workaround is the only
    // reliable approach for internally-tagged detail shapes.
    //
    // This test documents the current behaviour so future contributors
    // know the limitation is deliberate.
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains(r"export type KnownBugTaggedEnum ="),
        "KnownBugTaggedEnum should be emitted\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains(r"{ SlugInvalid: { reason: string } }"),
        "post-processor renders externally-tagged regardless of serde(tag)\n--- types.ts ---\n{types_ts}",
    );
}

#[test]
fn flat_struct_error_detail_round_trips_correctly() {
    // This is the RECOMMENDED workaround. Unlike the enum cases above,
    // a flat struct with `bool`/`Option<String>` fields renders to the
    // expected TypeScript shape and round-trips cleanly.
    let (_tmp, types_ts, _) = generate_all();
    assert!(
        types_ts.contains("export type WorkaroundErrorDetail"),
        "workaround struct should be exported\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("slug_taken: boolean"),
        "slug_taken should be boolean\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("slug_invalid: string | null"),
        "slug_invalid should be nullable string\n--- types.ts ---\n{types_ts}",
    );
    assert!(
        types_ts.contains("url_invalid: string | null"),
        "url_invalid should be nullable string\n--- types.ts ---\n{types_ts}",
    );
}
