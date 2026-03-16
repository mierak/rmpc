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
  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        lib = pkgs.lib;
        craneLib = crane.mkLib pkgs;
        # Only includes rust/cargo files in the build source, meaning that rebuilds won't happen for unrelated files.
        src = craneLib.cleanCargoSource ./.;

        # Common arguments shared across workspace dependency and package builds
        commonArgs = {
          inherit src;
          # Doesn't seem to be documented, but it was used in the crane quickstart.
          strictDeps = true;
        };

        # Build dependencies of the entire workspace, so that they can be re-used.
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs
          // {
            # To suppress warnings when building the workspace dependencies, see
            # https://github.com/ipetkov/crane/issues/281#issuecomment-1487845029
            # for other available workarounds.
            pname = "rmpc-workspace";
            version = "0.0.0";
          });

        # Common attributes shared between packages
        individualCrateArgs = pname:
          commonArgs
          // {
            # Use the shared built/cached workspace dependencies.
            inherit cargoArtifacts;

            # Sets pname and version of the package from keys extracted from the Cargo.toml
            inherit (craneLib.crateNameFromCargoToml {cargoToml = ./${pname}/Cargo.toml;}) version pname;

            # Build the specified package
            cargoExtraArgs = "-p ${pname}";
          };

        # Helper function to add files to the build source based on their file extension.
        workspace.root = ./.;
        sourcesWithExt = ext: lib.fileset.fileFilter (file: file.hasExt ext) workspace.root;

        # Main rust package, which can be run with `nix run github:mierak/rmpc`
        # (or just `nix run` if you're currently in this repository)
        rmpc = craneLib.buildPackage (
          individualCrateArgs "rmpc"
          # This "//" syntax just merges the two sets, preferring the values on the right-side set.
          // {
            # A little more complicated source, as non-rust/cargo files are needed for the
            # package/build.
            src = lib.fileset.toSource {
              inherit (workspace) root;
              fileset = lib.fileset.unions [
                # Will handle all rust/cargo related files.
                (craneLib.fileset.commonCargoSources workspace.root)
                # Ron files sourced for some commands.
                (sourcesWithExt "ron")
                # Default cover art.
                (sourcesWithExt "jpg")
                # Desktop file.
                (sourcesWithExt "desktop")
              ];
            };

            # Since the nix build is isolated, vergen won't have access to the git repository,
            # so we'll have to set the environment variable ourselves.
            #
            # We can't actually (easily) get the equivalent of `git describe` unfortunately,
            # see https://github.com/NixOS/nix/issues/7201
            VERGEN_GIT_DESCRIBE = self.shortRev or self.dirtyShortRev;

            # Additional files to be installed if the package is being, well, installed.
            nativeBuildInputs = with pkgs; [installShellFiles];
            postInstall = ''
              installManPage target/man/rmpc.1

              installShellCompletion --cmd rmpc \
                --bash target/completions/rmpc.bash \
                --fish target/completions/rmpc.fish \
                --zsh target/completions/_rmpc

              install -m 444 -D assets/rmpc.desktop $out/share/applications/rmpc.desktop
            '';
          }
        );

        # Can be run with `nix run github:mierak/rmpc#rmpcd` (or similar to above, instead run
        # `nix run .#rmpcd` if you're currently in this repository)
        rmpcd = craneLib.buildPackage (individualCrateArgs "rmpcd"
          // {
            # Like the rmpc package, we need to add extra files to the source.
            src = lib.fileset.toSource {
              inherit (workspace) root;
              fileset = lib.fileset.unions [
                # Will handle all rust/cargo related files.
                (craneLib.fileset.commonCargoSources workspace.root)
                # Builtin scripts
                (sourcesWithExt "lua")
              ];
            };

            # Run-time binary dependencies. We wrap the program to prepend the PATH with the
            # required binaries.
            nativeBuildInputs = with pkgs; [makeWrapper];
            postInstall = ''
              wrapProgram $out/bin/rmpcd \
                --prefix PATH : ${with pkgs;
                lib.makeBinPath [
                  # Provides `notify-send` binary for builtin notify script.
                  libnotify
                  # Provides `xdg-open` binary for builtin lastfm script.
                  xdg-utils
                  # For lyrics script, and probably other things too.
                  rmpc
                ]}
            '';
          });
      in {
        packages = {
          # Add the packages defined above to the packages set, so that they can be run/built.
          inherit rmpc rmpcd;

          # Make the default package just be rmpc.
          default = rmpc;
        };

        # Development shell, initialized either with `nix develop` or using direnv. Crane
        # automatically sets up all the rust-specific packages & environment variables,
        # so nothing else needs to be added.
        devShells.default = craneLib.devShell {
          # For some reason rust-analyzer isn't included in the default shell.
          buildInputs = with pkgs; [
            rust-analyzer
          ];
        };
      }
    );
}
