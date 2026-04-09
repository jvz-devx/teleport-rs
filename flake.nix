{
  description = "teleport-rs dev shell (Rust 1.91 MSRV + bun + CI tooling)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils
    , rust-overlay
    ,
    }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        # Pinned to the declared MSRV (`rust-version = "1.91"` in the
        # workspace Cargo.toml). This is exactly what CI's `msrv` job
        # runs, so `cargo check --workspace` inside this shell is a
        # faithful local reproduction.
        rustToolchain = pkgs.rust-bin.stable."1.93.0".default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "rustfmt"
            "clippy"
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          name = "teleport-rs";

          packages = with pkgs; [
            rustToolchain

            # JavaScript side. Bun natively handles the monorepo's
            # `workspace:*` protocol and hoists `tsc` into
            # `node_modules/.bin/` after `bun install`, which is what
            # the teleport-build snapshot test looks for.
            bun

            # Matches the CI `cargo-deny` job.
            cargo-deny

            # Optional dev-loop helpers.
            cargo-watch
            cargo-insta

            # Native build deps needed by a few transitive crates
            # (openssl-sys, ring, etc.).
            pkg-config
            openssl
          ];

          env = {
            RUST_BACKTRACE = "1";
          };

          shellHook = ''
            echo "teleport-rs dev shell"
            echo "  rustc : $(rustc --version)"
            echo "  cargo : $(cargo --version)"
            echo "  bun   : $(bun --version)"
            echo ""
            echo "Run the full CI check suite locally:"
            echo "  cargo fmt --all --check"
            echo "  cargo clippy --workspace --all-targets --all-features -- -D warnings"
            echo "  cargo test --workspace --all-features"
            echo "  RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps"
            echo "  cargo check --workspace --all-features   # MSRV smoke test"
            echo "  cargo deny --all-features check"
            echo "  bunx tsc --noEmit -p packages/client/tsconfig.json"
            echo "  bunx tsc --noEmit -p packages/vite/tsconfig.json"
            echo ""
          '';
        };
      });
}
