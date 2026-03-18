{
  craneLib,
  individualCrateArgs,
  lib,
  libnotify,
  makeWrapper,
  rmpc,
  sourcesWithExt,
  workspace,
  xdg-utils,
  # Options:
  #
  # Dependencies that should be included in the binary wrapper, making their binaries available at
  # run-time. These can be modified by calling `override` on the package.
  wrappedWithPkgs ? [
    # Provides `notify-send` binary for builtin notify script.
    libnotify
    # Provides `xdg-open` binary for builtin lastfm script.
    xdg-utils
    # For lyrics script, and probably other things too.
    rmpc
  ],
}:
craneLib.buildPackage (individualCrateArgs "rmpcd"
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
    nativeBuildInputs = [makeWrapper];
    postInstall = ''
      wrapProgram $out/bin/rmpcd \
        --prefix PATH : ${lib.makeBinPath wrappedWithPkgs}
    '';
  })
