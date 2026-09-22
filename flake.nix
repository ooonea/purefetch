{
  description = "purefetch — a fast, zero-dependency system information tool in Rust";

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
      packages = forAll (
        pkgs: rec {
          purefetch = pkgs.rustPlatform.buildRustPackage {
            pname = "purefetch";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            meta = {
              description = "A fast, fastfetch-style system information tool written entirely in Rust with zero dependencies";
              homepage = "https://github.com/ooonea/purefetch";
              license = with pkgs.lib.licenses; [
                mit
                asl20
                cc-by-40
                cc-by-sa-30
                gpl3Plus
              ];
              mainProgram = "purefetch";
              platforms = pkgs.lib.platforms.linux ++ pkgs.lib.platforms.darwin;
            };
          };
          default = purefetch;
        }
      );

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
