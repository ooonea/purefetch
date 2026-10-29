{
  description = "purefetch — terminal system information for Linux and macOS, with zero crate dependencies";

  # Only nixpkgs — no flake-utils — to keep the dependency graph as small as the
  # tool's own (which is empty).
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      # Linux via raw syscalls + /proc + /sys; macOS via libSystem FFI.
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        purefetch = pkgs.rustPlatform.buildRustPackage {
          pname = "purefetch";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          meta = {
            description = "Customizable terminal system information for Linux and macOS, written in Rust with zero crate dependencies";
            homepage = "https://github.com/ooonea/purefetch";
            license = with pkgs.lib.licenses; [
              mit
              asl20
              cc-by-40
              cc-by-sa-30
              cc-by-sa-40
              gpl3Plus
            ];
            mainProgram = "purefetch";
            platforms = pkgs.lib.platforms.linux ++ pkgs.lib.platforms.darwin;
          };
        };
        default = purefetch;
      });

      apps = forAll (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.system}.purefetch}/bin/purefetch";
        };
      });

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rustfmt
            pkgs.clippy
          ];
        };
      });

      overlays.default = final: prev: {
        purefetch = self.packages.${prev.system}.default;
      };
    };
}
