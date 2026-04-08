#![allow(clippy::expect_used, clippy::print_stdout)]

use specta::{ResolvedTypes, Types};
use teleport::ProcedureRegistration;
use teleport_build::{Config, HttpMethod as BuildHttpMethod, Naming, NamespaceStyle, ProcedureInfo};

// Pull in the procedure registrations from the main crate modules.
#[allow(unused_imports)]
use teleport_demo::{api, types};

const fn convert_method(method: teleport::HttpMethod) -> BuildHttpMethod {
    match method {
        teleport::HttpMethod::Get => BuildHttpMethod::Get,
        teleport::HttpMethod::Post => BuildHttpMethod::Post,
    }
}

fn collect_procedures() -> (Vec<ProcedureInfo>, ResolvedTypes) {
    let mut types = Types::default();
    let mut procedures = Vec::new();

    for reg in inventory::iter::<ProcedureRegistration> {
        let info = ProcedureInfo {
            name: reg.name(),
            method: convert_method(reg.method),
            path: reg.path(),
            doc: reg.doc.to_owned(),
            input_type: (reg.input_type)(&mut types),
            output_type: (reg.output_type)(&mut types),
            error_type: (reg.error_type)(&mut types),
        };
        procedures.push(info);
    }

    let resolved = ResolvedTypes::from_resolved_types(types);
    (procedures, resolved)
}

fn main() {
    let out_dir = std::env::var("TELEPORT_OUTPUT_DIR").map_or_else(
        |_| std::path::PathBuf::from("frontend/src/lib/api/generated"),
        std::path::PathBuf::from,
    );

    let config = Config {
        output_dir: out_dir,
        namespace_style: NamespaceStyle::default(),
        naming: Naming::default(),
        include_manifest: false,
        route_prefix: "/rpc".to_owned(),
        client_import_path: None,
    };

    let (procedures, resolved) = collect_procedures();

    println!("Generating TypeScript bindings for {} procedures...", procedures.len());
    for proc in &procedures {
        println!("  {} {} {}", proc.method.as_str(), proc.path, proc.name);
    }

    teleport_build::generate(&config, &procedures, &resolved)
        .expect("failed to generate TypeScript bindings");

    println!("Done! Output written to {}", config.output_dir.display());
}
