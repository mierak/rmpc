{
  description = "A modern, configurable, terminal based MPD Client with album art support via various terminal image protocols";

  inputs = {
    # For packages we pull.
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixpkgs-unstable";
    # So we don't have to manually define things for each os/arch combination.
    flake-utils.url = "github:numtide/flake-utils";
    # To cache rust crates.
    crane.url = "github:ipetkov/crane";
  };

  # NOTE: much of the following is taken from https://crane.dev/examples/quick-start-workspace.html.
  outputs = inputs: let
    # The "scope" produced from this function is used to automatically populate arguments to
    # functions we'll call to build packages and the devShell.
    mkScope = pkgs:
      pkgs.lib.makeScope pkgs.newScope (self: {
        inherit inputs;
        lib = pkgs.lib;
        craneLib = inputs.crane.mkLib pkgs;
        # Only includes rust/cargo files in the build source, meaning that rebuilds won't happen for unrelated files.
        src = self.craneLib.cleanCargoSource ./.;

        # Common arguments shared across workspace dependency and package builds
        commonArgs = {
          inherit (self) src;
          # Doesn't seem to be documented, but it was used in the crane quickstart.
          strictDeps = true;
        };

        # Build dependencies of the entire workspace, so that they can be re-used.
        cargoArtifacts = self.craneLib.buildDepsOnly (self.commonArgs
          // {
            # To suppress warnings when building the workspace dependencies, see
            # https://github.com/ipetkov/crane/issues/281#issuecomment-1487845029
            # for other available workarounds.
            pname = "rmpc-workspace";
            version = "0.0.0";
          });

        # Common attributes shared between packages
        individualCrateArgs = pname:
          self.commonArgs
          // {
            # Use the shared built/cached workspace dependencies.
            inherit (self) cargoArtifacts;

            # Sets pname and version of the package from keys extracted from the Cargo.toml
            inherit (self.craneLib.crateNameFromCargoToml {cargoToml = ./${pname}/Cargo.toml;}) version pname;

            # Build the specified package
            cargoExtraArgs = "-p ${pname}";
          };

        # Helper function to add files to the build source based on their file extension.
        workspace.root = ./.;
        sourcesWithExt = ext: self.lib.fileset.fileFilter (file: file.hasExt ext) self.workspace.root;

        # Main rust package, which can be run with `nix run github:mierak/rmpc`
        # (or just `nix run` if you're currently in this repository)
        rmpc = self.callPackage ./nix/rmpc.nix {};
        # Can be run with `nix run github:mierak/rmpc#rmpcd` (or similar to above, instead run
        # `nix run .#rmpcd` if you're currently in this repository)
        rmpcd = self.callPackage ./nix/rmpcd.nix {};

        # Development shell, initialized either with `nix develop` or using direnv.
        shell = self.callPackage ./nix/shell.nix {};
      });
  in
    inputs.flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = inputs.nixpkgs.legacyPackages.${system};
      in {
        packages = {
          # Add the packages defined above to the packages set, so that they can be run/built.
          inherit (mkScope pkgs) rmpc rmpcd;

          # Make the default package just be rmpc.
          default = (mkScope pkgs).rmpc;
        };

        devShells.default = (mkScope pkgs).shell;
      }
    );
}
