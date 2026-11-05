{ ... }:
{
  perSystem =
    { pkgs, self', ... }:
    let
      toolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "clippy"
          "rust-analyzer"
          "rust-src"
          "rustfmt"
        ];
      };
    in
    {
      devShells.default = pkgs.mkShell {
        packages = [
          toolchain
          pkgs.cargo-deny
          pkgs.cargo-nextest
          pkgs.just
          pkgs.pkg-config
          pkgs.sqlx-cli
          pkgs.typos
          # Credentials are declared in research-daemon/secretspec.toml; the
          # CLI is normally invoked through `secretspec run` (see justfile).
          pkgs.secretspec
          self'.packages.research
        ];

        RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
      };
    };
}
