# AGENTS.md

This repository takes a multipile diagram from its `.diagram` file to a picture. [README.md](README.md) says what each crate does and how a diagram is written.

## Piles

The repository declares piles: the top, and one for each crate. Read a pile's `.pile/intent.md` before you change the pile, and keep it true in the same commit.

## Working here

- `nix flake check` must pass before a commit: the build, every test, `clippy --deny warnings` and `rustfmt`. `nix develop` gives the same Rust it checks with.
- A change to what a file may say goes into `diagram-format`: a new word in the version it belongs to, and a new version beside the old ones when old files would read differently. Never change what an old version reads.
- A change to where a type puts its elements goes into its layout in `crates/document/src/layout/`, and comes with a test in `crates/pipeline/tests/types.rs` that says what the layout promises. A change to how anything looks or moves goes into the document (`paint.rs`, `theme.rs`), never into one writer: the page and the picture are both written from the document, and must keep showing the same thing.
- Every type has several cases in `crates/pipeline/cases/<type>/`. A new type, word or rule comes with a case that uses it, and a rule with a case in `cases/invalid/` whose first line says the finding it expects: `// expect: <code> at <line>:<column>`.
- Look at what you changed: the tests keep every page, picture and animation in `target/tmp/e2e/`, and `cargo run -p diagram-pipeline -- render <file> -o <out>.png` draws one.

## Commits

`main` changes only by a merged pull request, and CI runs `nix flake check` on each.
