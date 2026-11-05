{ inputs, ... }:
{
  perSystem =
    { pkgs, self', ... }:
    let
      toolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "clippy"
          "rust-src"
          "rustfmt"
        ];
      };

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain toolchain;

      src = craneLib.cleanCargoSource ../../research-daemon;

      commonArgs = {
        inherit src;
        pname = "research";
        version = "0.1.0";
        cargoLock = ../../research-daemon/Cargo.lock;
        strictDeps = true;
      };
    in
    {
      packages.research = craneLib.buildPackage commonArgs;
      packages.default = self'.packages.research;
    };
}
