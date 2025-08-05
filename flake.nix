{
  description = "Launcher-driven audio manager for Linux";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];

        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "pwmenu";
          version = self.shortRev or self.dirtyShortRev or "unknown";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            clang
            llvmPackages.libclang
          ];

          buildInputs = with pkgs; [
            pipewire.dev
          ];

          doCheck = true;
          CARGO_BUILD_INCREMENTAL = "false";
          RUST_BACKTRACE = "full";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

          meta = {
            description = "Launcher-driven audio manager for Linux";
            homepage = "https://github.com/e-tho/pwmenu";
            license = pkgs.lib.licenses.gpl3Plus;
            maintainers = [
              {
                github = "e-tho";
              }
            ];
            mainProgram = "pwmenu";
          };
        };

        devShells.default =
          with pkgs;
          mkShell {
            nativeBuildInputs = [
              pkg-config
              clang
              llvmPackages.libclang
              (rust-bin.stable.latest.default.override {
                extensions = [ "rust-src" ];
              })
            ];

            buildInputs = [
              pipewire.dev
            ];

            inherit (self.packages.${system}.default)
              CARGO_BUILD_INCREMENTAL
              RUST_BACKTRACE
              LIBCLANG_PATH
              ;
          };
      }
    );
}
