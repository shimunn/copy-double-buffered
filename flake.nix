{
  description = "Description for the project";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ flake-parts, self, crane, rust-overlay, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        # To import a flake module
        # 1. Add foo to inputs
        # 2. Add foo as a parameter to the outputs function
        # 3. Add here: foo.flakeModule

      ];
      systems =
        [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      perSystem = { config, self', inputs', pkgs, system, ... }:
        let
          inherit (pkgs) lib;
          toolchainFile = "${self}/rust-toolchain.toml";
          craneLib = (self.inputs.crane.mkLib pkgs).overrideToolchain (p:
            if builtins.pathExists toolchainFile then
              p.rust-bin.fromRustupToolchainFile toolchainFile
            else
              p.rust-bin.nightly.latest.default.override { extensions = [ "rust-src" "rust-analyzer" ]; });
          src = self;

          # Common arguments can be set here to avoid repeating them later
          commonArgs = {
            inherit src;
            strictDeps = true;

            buildInputs = [
	      pkgs.openssl
	      pkgs.openssl.dev
	      pkgs.pkg-config 
              # Add additional build inputs here
            ] ++ lib.optionals pkgs.stdenv.isDarwin [
              # Additional darwin specific inputs can be set here
              pkgs.libiconv
            ];
	    nativeBuildInputs = [ ];
            # Additional environment variables can be set directly
            # MY_CUSTOM_VAR = "some value";
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          my-crate =
            craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
          my-crate-doc =
            craneLib.cargoDoc (commonArgs // { inherit cargoArtifacts; });
        in {

          _module.args.pkgs = import self.inputs.nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          # Per-system attributes can be defined here. The self' and inputs'
          # module parameters provide easy access to attributes of the same
          # system.

          packages = { inherit cargoArtifacts my-crate my-crate-doc; };
          checks = { };
          devShells.default = craneLib.devShell {
            # Inherit inputs from checks.
            checks = self.checks.${system};
	    inherit (commonArgs) buildInputs nativeBuildInputs;
          };
        };
      flake = {
        # The usual flake attributes can be defined here, including system-
        # agnostic ones like nixosModule and system-enumerating ones, although
        # those are more easily expressed in perSystem.

      };
    };
}
