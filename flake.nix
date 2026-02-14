{
  description = "nixmate - NixOS Multi-Tool TUI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
        ];

        buildInputs = with pkgs; [
        ];
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "nixmate";
          version = "0.7.2";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          inherit nativeBuildInputs buildInputs;

          meta = with pkgs.lib; {
            description = "NixOS Multi-Tool – All your Nix utilities in one TUI";
            homepage = "https://github.com/daskladas/nixmate";
            license = licenses.mit;
            maintainers = [ ];
            platforms = platforms.linux;
            mainProgram = "nixmate";
          };
        };

        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/nixmate";
        };
      }
    );
}
