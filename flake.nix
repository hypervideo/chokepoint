{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };


  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        chokepoint = pkgs.writeShellScriptBin "chokepoint" ''
          #!${pkgs.stdenv.shell}
          cargo run -p chokepoint-cli --bin chokepoint --release -q -- "$@"
        '';
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            clippy
            clang
            pkg-config
          ];

          buildInputs = with pkgs; [
            openssl
          ];

          packages = with pkgs; [
            rust-analyzer
            (rustfmt.override { asNightly = true; })
            cargo-nextest
            cargo-readme
            graph-cli
            chokepoint
          ];

          RUST_BACKTRACE = "1";
          RUST_LOG = "debug";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };
      }
    );
}
