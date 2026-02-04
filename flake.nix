{
  description = "A modern, configurable, terminal based MPD Client with album art support via various terminal image protocols";

  inputs = {
    # For packages we pull.
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixpkgs-unstable";
    # So we don't have to manually define things for each os/arch combination.
    flake-utils.url = "github:numtide/flake-utils";
    # To cache rust crates.
    naersk = {
      url = "github:nix-community/naersk";
      # Use the same nixpkgs that we've defined above.
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    naersk,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        naerskLib = pkgs.callPackage naersk {};
      in {
        # Rust package, which can be run with `nix run github:mierak/rmpc`
        # (or without the "github:..." if you're currently in this repository)
        packages.default = naerskLib.buildPackage {
          src = ./.;
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
        };

        # Development shell, initialized either with `nix develop` or using direnv
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
          ];

          # Needed so programs like rust-analyzer can find the rust source code.
          env.RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };
      }
    );
}
