{
  cava,
  craneLib,
  individualCrateArgs,
  inputs,
  installShellFiles,
  lib,
  makeWrapper,
  sourcesWithExt,
  workspace,
  # Options:
  #
  # Dependencies that should be included in the binary wrapper, making their binaries available at
  # run-time. These can be modified by calling `override` on the package.
  wrappedWithPkgs ? [
    # For cava visualization
    cava
  ],
}:
craneLib.buildPackage (
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
    VERGEN_GIT_DESCRIBE = inputs.self.shortRev or inputs.self.dirtyShortRev;

    # Additional files to be installed if the package is being, well, installed.
    nativeBuildInputs = [installShellFiles makeWrapper];
    postInstall = ''
      wrapProgram $out/bin/rmpc \
        --prefix PATH : ${lib.makeBinPath wrappedWithPkgs}

      installManPage target/man/rmpc.1

      installShellCompletion --cmd rmpc \
        --bash target/completions/rmpc.bash \
        --fish target/completions/rmpc.fish \
        --zsh target/completions/_rmpc

      install -m 444 -D assets/rmpc.desktop $out/share/applications/rmpc.desktop
    '';
  }
)
