{
  description = "multipile's diagrams: a .diagram file read, built into self-contained HTML, and rendered with takumi";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      crane,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAll =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          let
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            };
            # One Rust for building, checking and working here.
            toolchain = pkgs.rust-bin.stable."1.99.1".default.override {
              extensions = [
                "clippy"
                "rustfmt"
                "rust-src"
              ];
            };
            craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
            src = pkgs.lib.cleanSourceWith {
              src = ./.;
              # The crates read more than Cargo's own files: the fonts they
              # embed and their licence, the cases the tests run, and the
              # Markdown the docs include.
              filter =
                path: type:
                (craneLib.filterCargoSources path type)
                || pkgs.lib.hasSuffix ".diagram" path
                || pkgs.lib.hasSuffix ".woff2" path
                || pkgs.lib.hasSuffix ".txt" path
                || pkgs.lib.hasSuffix ".md" path;
            };
            common = {
              inherit src;
              strictDeps = true;
              pname = "diagrams";
              version = "0.1.0";
              # `nix run` runs the diagram command.
              meta.mainProgram = "diagram";
            };
            cargoArtifacts = craneLib.buildDepsOnly common;
            # The cases as data, for another repository's tests: every case
            # file and cases.json, which lists them. cases.nix says its
            # shape. It is the same on every system.
            cases = pkgs.callPackage ./cases.nix { };
          in
          f {
            inherit
              pkgs
              toolchain
              craneLib
              common
              cargoArtifacts
              cases
              ;
          }
        );
    in
    {
      packages = forAll (
        { craneLib, common, cargoArtifacts, cases, ... }:
        {
          # The diagram command, built with every test run.
          default = craneLib.buildPackage (common // { inherit cargoArtifacts; });
          inherit cases;
        }
      );

      checks = forAll (
        { pkgs, craneLib, common, cargoArtifacts, cases, ... }:
        {
          # The same derivation as packages.default, so it is built once.
          build = craneLib.buildPackage (common // { inherit cargoArtifacts; });
          test = craneLib.cargoTest (common // { inherit cargoArtifacts; });
          clippy = craneLib.cargoClippy (
            common
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--workspace --all-targets -- --deny warnings";
            }
          );
          fmt = craneLib.cargoFmt { inherit (common) src pname version; };
          # Building the cases refuses a case that is malformed. This holds
          # what was built to the files: the manifest lists every file the
          # directory holds, and holds every file it lists.
          cases = pkgs.runCommand "diagram-cases-check" { nativeBuildInputs = [ pkgs.jq ]; } ''
            cd ${cases}
            jq -r '.cases[] | .path // (.before, .after)' cases.json | sort > $TMPDIR/listed
            find . -type f ! -path ./cases.json | sed 's|^\./||' | sort > $TMPDIR/held
            if ! diff $TMPDIR/listed $TMPDIR/held; then
              echo "cases.json and the files beside it differ: < is listed only, > is a file only" >&2
              exit 1
            fi
            for kind in valid invalid diff; do
              jq -e --arg kind "$kind" 'any(.cases[]; .kind == $kind)' cases.json > /dev/null \
                || { echo "cases.json lists no $kind case" >&2; exit 1; }
            done
            touch $out
          '';
        }
      );

      devShells = forAll (
        { pkgs, toolchain, ... }:
        {
          default = pkgs.mkShell { packages = [ toolchain ]; };
        }
      );

      formatter = forAll ({ pkgs, ... }: pkgs.nixfmt);
    };
}
