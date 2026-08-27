{
  description = "Merman headless Mermaid-compatible CLI";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      packageFor = system: import ./default.nix { pkgs = nixpkgs.legacyPackages.${system}; };
    in
    {
      packages = forAllSystems (system: {
        default = packageFor system;
        merman-cli = packageFor system;
      });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/merman-cli";
        };
      });

      checks = forAllSystems (system: {
        package = self.packages.${system}.default;
        source-contract =
          let
            package = self.packages.${system}.default;
            source = package.source;
            scriptFiles = package.sourcePolicy.script_files;
          in
          nixpkgs.legacyPackages.${system}.runCommand "merman-cli-source-contract" { } ''
            test -f ${source}/Cargo.toml
            test -f ${source}/Cargo.lock
            test -f ${source}/crates/merman-cli/assets/completions/merman-cli.bash
            test -f ${source}/capabilities/artifact-profiles-v1.json
            ${nixpkgs.lib.concatMapStringsSep "\n" (relative: "test -f ${source}/${relative}") scriptFiles}
            test ! -e ${source}/.git
            test ! -e ${source}/repo-ref
            test ! -e ${source}/target
            test ! -e ${source}/platforms
            test ! -e ${source}/tools
            test ! -e ${source}/crates/merman-node
            test ! -e ${source}/crates/merman-node/target
            test ! -e ${source}/crates/merman-wasm/target
            test "$(find ${source}/scripts -mindepth 1 -maxdepth 1 | wc -l)" -eq ${
              toString (builtins.length scriptFiles)
            }
            touch "$out"
          '';
      });
    };
}
