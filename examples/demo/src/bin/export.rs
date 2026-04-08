#![allow(clippy::expect_used, clippy::print_stdout)]

use teleport_build::{Config, Naming, NamespaceStyle};

// Pull in the procedure registrations from the main crate modules.
#[allow(unused_imports)]
use teleport_demo::{api, types};

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

    println!("Generating TypeScript bindings...");

    teleport_build::export_from_inventory(&config)
        .expect("failed to generate TypeScript bindings");

    println!("Done! Output written to {}", config.output_dir.display());
}
