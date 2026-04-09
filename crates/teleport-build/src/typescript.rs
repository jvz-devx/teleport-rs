// Specta DataType -> TypeScript type generation.
//
// Converts collected procedure types into `types.ts` content.

use specta::ResolvedTypes;
use specta_typescript::Typescript;

use crate::GenerateError;

/// Generate the contents of `types.ts` from the resolved type collection.
///
/// Uses `specta-typescript` to render all named types (structs, enums)
/// that were registered by the export binary.
pub fn generate_types(resolved_types: &ResolvedTypes) -> Result<String, GenerateError> {
    let ts = Typescript::default().header(crate::GENERATED_HEADER);
    ts.export(resolved_types)
        .map_err(|e| GenerateError::TypeExport(e.to_string()))
}

#[cfg(test)]
mod tests {
    // Test-only: `.expect()` is informative and any panic is caught by the test runner.
    #![allow(clippy::expect_used)]

    use super::*;
    use specta::Types;

    #[derive(Debug, Clone, specta::Type)]
    #[allow(dead_code)]
    struct TestUser {
        id: String,
        name: String,
        email: String,
    }

    #[test]
    fn generates_interface_for_struct() {
        let types = Types::default().register::<TestUser>();
        let resolved = ResolvedTypes::from_resolved_types(types);
        let output = generate_types(&resolved).expect("should generate types");

        assert!(output.contains("export type TestUser"));
        assert!(output.contains("id"));
        assert!(output.contains("name"));
        assert!(output.contains("email"));
        assert!(output.contains(crate::GENERATED_HEADER));
    }

    #[test]
    fn empty_types_produces_header_only() {
        let types = Types::default();
        let resolved = ResolvedTypes::from_resolved_types(types);
        let output = generate_types(&resolved).expect("should generate types");

        assert!(output.contains(crate::GENERATED_HEADER));
    }
}
