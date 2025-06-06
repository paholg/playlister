{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      crane,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [
          (import rust-overlay)
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rust = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-analyzer"
            "rust-src"
          ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain (p: p.rust-bin.stable.latest.default);

        buildInputs = with pkgs; [ openssl ];
        nativeBuildInputs = with pkgs; [ pkg-config ];

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        crate = craneLib.buildPackage {
          inherit buildInputs nativeBuildInputs;
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;
          meta.mainProgram = cargoToml.package.name;
        };
      in
      {
        checks = {
          inherit crate;
        };
        packages = {
          default = crate;
          inherit crate;
        };
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          packages = [
            pkgs.just
            pkgs.python313
            pkgs.python313Packages.tidalapi
            rust
          ];

          env = {
            CACHE_DIR = "cache";
            LD_LIBRARY_PATH = nixpkgs.lib.makeLibraryPath buildInputs;
            RUST_BACKTRACE = 1;
          };
        };
      }
    );
}
