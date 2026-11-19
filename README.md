# diagrams

multipile's diagrams, from a file to a picture. A diagram is a `.diagram` file, written in [KDL](https://kdl.dev), that says what a diagram shows and never where. This repository reads it, paints it into one complete document, and writes that document as a self-contained HTML page with its CSS and its animation, or renders it to a still or an animated image with [takumi](https://github.com/kane50613/takumi), with no browser and nothing parsed.

> [!NOTE]
> **multipile is in early development.** Commands, file formats and output can still change between releases. Try it, and [tell us what you find](https://github.com/multipile/diagrams/issues).

```kdl
diagram dependency version=1 {
  title "How the crates depend on one another"
  desc "The cli crate depends on piles and storage. Piles depends on storage."

  box cli sub="crates/pile"
  box piles sub="crates/piles"
  box storage sub="crates/storage" focal=#true

  line cli piles
  line cli storage
  line piles storage
}
```

## The pipeline

```
.diagram ─ format ─▶ diagram ─ document ─▶ document ─┬─ html   ─▶ a page, or an element to embed
                                                     └─ render ─▶ takumi's nodes ─▶ PNG, WebP, GIF, APNG
```

| Step | Crate | What it does |
| --- | --- | --- |
| Read | [`diagram-format`](crates/format) | Reads a `.diagram` file into a diagram, by the version of the format it says, and holds it to the rules every diagram and its type are held to. A file that breaks one is refused with every finding, each at its line and column, with what fixes it. It keeps the file's metadata as written, and never reads it. |
| Document | [`diagram-document`](crates/document) | Lays the diagram out by its type's rules, and paints it into its complete document: every node placed and styled down to its colours, borders and fonts, lines drawn as boxes, and every animation (a reveal, or the diff of two versions) a track of keyframes. |
| Page | [`diagram-html`](crates/html) | Writes the document as HTML that loads and runs nothing: a whole page, or one element to embed. Each node is a `<div>` with its style inline, and each track CSS keyframes. |
| Picture | [`diagram-render`](crates/render) | Builds the document as takumi's nodes, property by property, and renders it to PNG or WebP, or frame by frame to an animated GIF, WebP or APNG. Takumi parses no HTML, SVG or CSS. |
| End to end | [`diagram-pipeline`](crates/pipeline) | The steps as one call, the `diagram` command, and the tests that run every case through every step. |

A page and a picture of one document show the same thing: the renderer works out each frame from the same tracks the page's keyframes are written from, on the same curves.

## The format

A file holds one `diagram` node, which names its type and the version of the format: `diagram <type> version=1 { … }`. Every version stays readable: a new version, or another format, is read into the same diagram beside the old ones, so a file never has to be rewritten because the format moved.

Every diagram has a `title` (at most 60 characters), a `desc` (one sentence, for a screen reader) and may have a `note`, drawn under it. A box is written the same way in every type:

```kdl
box <key> "<name>" sub="<a second line>" as=plain focal=#true
```

`as` is `plain`, `store`, `external`, `input`, `optional` or `security`. A box with no name shows its key. At most two elements of a diagram are focal. A name is at most 24 characters, a second line 32, and a line's label 14.

| Type | Shows | Its own nodes | Limits |
| --- | --- | --- | --- |
| `architecture` | the parts of a system, and what talks to what | `flow`, `box` (which may hold up to 3 `box`), `tier <key>… zone="…" trust=#true`, `line <from> <to> label="…" kind=call\|link\|async\|optional` | 9 boxes, 12 lines, 3 zones |
| `sequence` | messages between actors, in time order | `actor`, `call`/`reply`/`notify <from> <to> "<label>"`, `self <actor> "<label>"`, `opt`/`loop "<guard>" { … }`, `alt { case "<guard>" { … } case "else" { … } }` | 5 actors, 12 messages, 1 fragment, none inside another |
| `state` | states, and the events that move between them | `flow`, `state <key> "<name>" start=#true end=#true`, `transition <from> <to> on="…" guard="…" action="…"`, `any <to> on="…"` | 9 states, 12 transitions |
| `swimlane` | the steps of a process, by who does each | `lane <key> "<name>" { step … }`, `line` | 5 lanes, 12 steps, 12 lines |
| `process` | the same, with each step's tool and data | `column <key> "<label>"`, `lane <key> "<name>" badge=ABC { task <key> "<title>" column=<key> tool="…" in=file out=table }`, `line <from> <to> trigger=#true` | 6 lanes, 12 columns, 1 focal column and task |
| `tree` | a parent and its children | `flow`, `item "<name>" { item … }` | 4 levels, 5 children of one parent |
| `dependency` | what depends on what | `flow`, `box`, `line <from> <to>`, `cycle <from> <to>` | 9 boxes, 14 lines, 4 ranks, 1 cycle |
| `nested` | scopes inside scopes | `level "<label>" { level … }` | 3 to 6 levels |
| `treemap` | how a whole splits into parts, by size | `measure "…"`, `source "…"`, `part <key> "<name>" value=<n> short=ABC` | 4 to 8 parts |

[`crates/pipeline/cases/`](crates/pipeline/cases) holds several diagrams of every type, files that must be refused, and pairs to compare.

### Metadata

A diagram can carry what its host wants to say about it: which page documents a box, which team owns a service, what the diagram was drawn from. This is **metadata**. The diagrams keep it exactly as it is written and never read it. The host that owns it gives it its meaning, and checks it.

```kdl
box storage docs:page="storage.md" docs:reviewed=#true
line cli storage docs:section=3

meta docs {
  index "crates.md" depth=2
  owners {
    team "storage" since="2026-01-12"
  }
}
```

- **On an element or a line**, metadata is a property named `<owner>:<key>`: two words of letters, digits, `_` and `-`, around one `:`. Its value is text, a number, `#true` or `#false`. Every element a type names takes it (a box, an actor, a state, a step, a task, a tree's item, a level, a part), and so does a `line` of an architecture, a swimlane or a dependency diagram, and a dependency's `cycle`.
- **For the whole diagram**, metadata is a `meta <owner> { … }` node inside `diagram`, at most one for each owner. It holds any KDL: nodes with arguments, properties and children.

The owner is the first word, so two hosts can write metadata in one file without reading each other's.

Only what makes metadata unreadable is refused: `meta-invalid` for a name that is not two words around one `:`, a value that is `#null`, or a `meta` that does not name its one owner, and `meta-twice` for a second block of one owner. Nothing inside metadata is checked, so a file that reads here may still be refused by its host.

A reader gets metadata with where it is written. For a string, it also gets where each of the string's lines starts in the file, so a host that keeps a small language in a string (in a raw string, `#"…"#`, or a multi-line one, `#"""…"""#`) can report a mistake at its line and column in the `.diagram` file.

A page writes each property of an element or a line on that element's `<div>`, as `data-<owner>-<key>="<value>"`: `data-docs-page="storage.md"`. A `meta` node is not written to the page, and no metadata changes a picture. Metadata is no part of a comparison either: two versions are compared by what is drawn, so an element whose metadata alone changed is not reported. A host that finds in its own metadata that an element now stands for something else reports it as changed, and it is drawn as changed.

## Use it

### From the command line

```sh
nix run github:multipile/diagrams -- check my.diagram
nix run github:multipile/diagrams -- render my.diagram -o my.png          # a picture
nix run github:multipile/diagrams -- render my.diagram -o my.gif          # revealed, element by element
nix run github:multipile/diagrams -- render my.diagram -o my.html         # a page that loads nothing
nix run github:multipile/diagrams -- diff before.diagram after.diagram -o change.gif
```

`render` and `diff` take `--at <ms>` for one moment of an animation, `--fps` and `--scale`. In a clone, `cargo run -p diagram-pipeline --` runs the same command.

`render --changed <keys>` draws a diagram with some of it marked as changed, the way a diff marks a change: an element by its key, and a line by the keys of its two ends.

```sh
nix run github:multipile/diagrams -- render my.diagram -o my.png --changed 'storage,cli->storage'
```

`diff --changed <keys>` takes the same list, and adds those to what is drawn differently.

### From Rust

Take the crates of the steps you need from this repository, at a commit:

```toml
[dependencies]
diagram-format = { git = "https://github.com/multipile/diagrams", rev = "<commit>" }
diagram-document = { git = "https://github.com/multipile/diagrams", rev = "<commit>" }
diagram-html = { git = "https://github.com/multipile/diagrams", rev = "<commit>" }
diagram-render = { git = "https://github.com/multipile/diagrams", rev = "<commit>" }   # only to render pictures
```

```rust
let read = diagram_format::read(&source)?;
let document = diagram_document::document(&read.diagram, diagram_document::Animation::Reveal);
let page = diagram_html::page(&document, &diagram_html::Options::default());
let gif = diagram_render::Renderer::new().animation(&document, diagram_render::MotionFormat::Gif, 25)?;
```

A host reads a diagram's metadata from `read.diagram`: `meta` on each element and line, and `read.diagram.meta` for the blocks.

Two versions are compared, and the comparison drawn, in two steps, so that the host decides what changed:

```rust
use diagram_document::Subject;

// What is drawn differently, as plain data: `added`, `removed` and `changed`.
let mut report = diagram_document::compare(&before, &after)?;
// The host's own decision: this box is drawn the same, and stands for something else.
report.change(Subject::element("storage"));
report.change(Subject::line("cli", "storage"));
// Exactly what the report says, still or animated.
let document = diagram_document::diff_document(&before, &after, &report, false)?;
```

A report names an element by its key (`Subject::Element`), a line by the keys of its two ends (`Subject::Line`), and anything a file names by no key, such as a lane or a message, by the id its layout gives it (`Subject::Shape`). The drawing decides nothing: an element the report does not list is drawn as unchanged, though the two versions draw it differently, and one it lists as changed is marked, though they draw it the same.

`diagram_document::changed_document(&read.diagram, &report)` draws one diagram with what a report says marked, for a diagram whose file did not change though what it shows did.

Reading, painting and writing pages adds about 0.8 MB to a release binary. Rendering pictures adds about 5 MB more, most of it takumi.

### From Nix

The flake gives the `diagram` command as `packages.default`, and pins the Rust it is built and checked with:

```nix
inputs.diagrams.url = "github:multipile/diagrams";
```

It also gives the cases as data, as `packages.cases`, for a repository that draws diagrams with these crates and wants to test itself on every diagram they promise something about:

```sh
nix build github:multipile/diagrams#cases
```

The directory it builds holds every file of [`crates/pipeline/cases/`](crates/pipeline/cases), at the same path, and `cases.json`, which lists them. The manifest is made from the files when the directory is built: a case is added by adding its file. A flake takes the directory as it is, or builds its own from it.

```json
{
  "cases": [
    { "kind": "valid", "group": "tree", "name": "a-pile-directory", "type": "tree",
      "path": "tree/a-pile-directory.diagram" },
    { "kind": "invalid", "group": "invalid", "name": "cell-taken",
      "path": "invalid/cell-taken.diagram",
      "expect": { "code": "cell-taken", "line": 8, "column": 5 } },
    { "kind": "diff", "group": "diff", "name": "a-crate-is-added",
      "before": "diff/a-crate-is-added/before.diagram",
      "after": "diff/a-crate-is-added/after.diagram" }
  ]
}
```

| Kind | What the case is | What it carries |
| --- | --- | --- |
| `valid` | a diagram that must read and draw | `type`, its diagram type, and `path` |
| `invalid` | a file that must be refused | `path`, and `expect`: the finding its first line says, as `code`, `line` and `column` |
| `diff` | two versions of one diagram, to compare | `before` and `after` |

Every case has its `group`, the directory it is in, and its `name`. Every path is relative to the directory. [`cases.nix`](cases.nix) builds it, and refuses a case that is malformed: a file in no group, a file to refuse that does not say the finding it expects, or a pair with one version missing.

## Check it

```sh
nix develop         # the pinned Rust
nix flake check     # build, every test, clippy, rustfmt and the cases as data, as CI does
cargo test          # the same tests
```

The end-to-end tests keep what they make in `target/tmp/e2e/`: each case's page, picture, reveal and diff, to look at.

CI runs `nix flake check` on every pull request ([`check`](.github/workflows/check.yml)). A second workflow, [`test diagrams`](.github/workflows/test-diagrams.yml), renders every case as a picture and every pair as its diff, and shows them in the pull request's description, between `<!-- test-diagrams -->` and `<!-- /test-diagrams -->`, with each picture no run had stored before marked new. It stores each picture on the `assets/test-diagrams` branch, named by the SHA-256 of its bytes, so a picture is stored once. A pull request from a fork has a read-only token: its pictures are rendered, and not shown.

## Where the rules come from

The diagram types, the rules every diagram is held to, and each type's limits are adapted from [Diagram Design](https://github.com/cathrynlavery/diagram-design), an agent skill by Cathryn Lavery: its catalogue of diagram types, each with when to use it, how it is laid out and how much it may hold, and its drawing rules (one idea per diagram, at most 9 boxes, 12 lines and 2 accents, lines at right angles on a 4px grid, labels of at most 14 characters). Its idea of a diff that keeps one layout for both versions, and matches elements by a stable id, shaped how this repository draws a change.

This repository takes no code from it. Its licence is in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Fonts and licence

Diagrams are drawn in Geist and Geist Mono, by the Geist Project Authors, under the SIL Open Font License 1.1 ([`crates/document/fonts/OFL.txt`](crates/document/fonts/OFL.txt)). A page embeds them, and the renderer registers them, since takumi loads no system fonts.

The code is under the MIT licence ([LICENSE](LICENSE)).
