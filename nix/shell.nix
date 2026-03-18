{
  craneLib,
  rust-analyzer,
  rustfmt,
}:
# Crane automatically sets up all the rust-specific packages & environment variables, so nothing
# else needs to be added.
#
# Use nightly rustfmt, as it's required for almost all the configured formatting options.
(craneLib.overrideScope (_: _: {rustfmt = rustfmt.override {asNightly = true;};})).devShell {
  # rust-analyzer isn't included in the default shell, because it's not provided in the default profile:
  # https://rust-lang.github.io/rustup/concepts/profiles.html
  buildInputs = [
    rust-analyzer
  ];
}
