use std::fmt::Write as _;

use teleport_contract::ContractBundle;

use crate::GENERATED_HEADER;
use crate::GenerateError;
use crate::config::Config;

/// Generate the contents of `errors.ts`.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn generate_errors(
    bundle: &ContractBundle,
    config: &Config,
) -> Result<String, GenerateError> {
    let client_path = config
        .client_import_path
        .as_deref()
        .unwrap_or("@teleport-rs/client");

    let mut out = String::with_capacity(1024);
    out.push_str(GENERATED_HEADER);
    out.push('\n');

    let mut aliases = Vec::new();
    let mut type_imports = Vec::new();

    for proc in &bundle.procedures {
        let error_ts = if matches!(&proc.error_type, teleport_contract::TypeExpr::Tuple(elements) if elements.is_empty())
        {
            "never".to_owned()
        } else {
            crate::ts_utils::type_expr_to_ts(&proc.error_type)
        };

        if error_ts == "never" {
            continue;
        }

        let alias_name = format!("{}Error", crate::naming::snake_to_pascal(&proc.method_name));
        aliases.push((alias_name, error_ts.clone()));
        if !crate::ts_utils::is_ts_primitive(&error_ts) {
            type_imports.push(error_ts);
        }
    }

    if !aliases.is_empty() {
        out.push('\n');
        let _ = writeln!(out, "import type {{ AppError }} from \"{client_path}\";");
    }

    if !type_imports.is_empty() {
        type_imports.sort();
        type_imports.dedup();
        let _ = writeln!(
            out,
            "import type {{ {} }} from \"./types\";",
            type_imports.join(", ")
        );
    }

    if !aliases.is_empty() {
        out.push('\n');
        out.push_str("// Procedure-specific error aliases\n");
        for (alias_name, error_ts) in &aliases {
            let _ = writeln!(out, "export type {alias_name} = AppError<{error_ts}>;");
        }
    }

    Ok(out)
}
