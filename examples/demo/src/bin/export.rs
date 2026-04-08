#![allow(clippy::expect_used, clippy::print_stdout)]

use teleport_build::Config;

// Pull in the procedure registrations from the main crate modules.
#[allow(unused_imports)]
use teleport_demo::{api, types};

fn main() {
    let out_dir = std::env::var("TELEPORT_OUTPUT_DIR").map_or_else(
        |_| std::path::PathBuf::from("frontend/src/lib/api/generated"),
        std::path::PathBuf::from,
    );

    println!("Generating TypeScript bindings...");

    teleport_build::export_from_inventory(&Config::new(out_dir))
        .expect("failed to generate TypeScript bindings");

    println!("Done!");
}
