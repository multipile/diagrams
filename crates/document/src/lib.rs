//! A diagram as a complete document, which every output is written from.
//!
//! A layout places every element of a diagram by its type's rules
//! ([`layout`]), into a [`Scene`]. The scene is painted into a [`Document`]
//! ([`document()`]): every node it draws, placed and styled down to its
//! colours, borders and fonts, and every animation a track of keyframes. A
//! writer only translates a document: `diagram-html` into a page, and
//! `diagram-render` into takumi's nodes, so both draw the same thing.
//!
//! ```
//! let read = diagram_format::read(r#"
//! diagram dependency version=1 {
//!   title "How the crates depend on one another"
//!   desc "The cli crate depends on storage."
//!   box cli
//!   box storage
//!   line cli storage
//! }
//! "#).unwrap();
//! let document = diagram_document::document(&read.diagram, diagram_document::Animation::Reveal);
//! assert!(document.walk().iter().any(|node| node.id.as_deref() == Some("box:storage")));
//! assert!(document.duration_ms > 0);
//! ```

pub mod diff;
pub mod document;
pub mod fonts;
pub mod geometry;
pub mod layout;
pub mod paint;
pub mod scene;
pub mod theme;

use diagram_format::Diagram;

pub use diff::Diff;
pub use document::Document;
pub use layout::layout;
pub use scene::Scene;

/// How a document moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Animation {
    /// It does not: the finished picture.
    #[default]
    None,
    /// Its elements appear in the order their layout gives: a dependency
    /// diagram rank by rank, a sequence message by message.
    Reveal,
}

/// A diagram's document.
pub fn document(diagram: &Diagram, animation: Animation) -> Document {
    paint::document(&layout(diagram), animation)
}

/// What [`changed_document`] asks about: one element or one line of a
/// diagram, named by the keys the file gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark<'a> {
    /// An element the file names by a key: a box, an actor, a state, a
    /// step, a task, a part. A tree's item is named by the names on its
    /// path from the root, joined by ` / `, and a nested diagram's level by
    /// `level-1` for the outermost, `level-2` for the next.
    Element(&'a str),
    /// A `line` or a `cycle`, by the keys of the two elements it joins.
    Line { from: &'a str, to: &'a str },
}

/// A diagram's document with what a change touched marked: each element
/// and each line `changed` says has changed is outlined and badged as a
/// still diff marks a change, and listed under the diagram. `changed` is
/// asked about every element the file names by a key, and about every
/// `line` and `cycle`. What makes one changed is its caller's to say: the
/// diagram knows only its keys.
///
/// This is how a review shows a diagram whose own file a change did not
/// touch, though it touched what the diagram shows.
pub fn changed_document(diagram: &Diagram, changed: impl Fn(Mark) -> bool) -> Document {
    use scene::{Element, Shape, Status};
    let scene = layout(diagram);
    let elements = scene
        .elements
        .iter()
        .map(|element| {
            let mark = match &element.shape {
                Shape::Box(shape) => shape.key.as_deref().map(Mark::Element),
                Shape::Region(shape) => shape.key.as_deref().map(Mark::Element),
                Shape::Line(shape) => shape.ends.as_ref().map(|(from, to)| Mark::Line { from, to }),
                _ => None,
            };
            let status = if mark.is_some_and(&changed) {
                Status::Changed
            } else {
                Status::Same
            };
            diff::DiffElement {
                element: Element {
                    status,
                    ..element.clone()
                },
                role: diff::Role::Kept,
            }
        })
        .collect();
    paint::diff_document(&Diff { scene, elements }, false)
}

/// Why two diagrams cannot be drawn as one diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffError {
    /// They are of different types, so no element of one is an element of
    /// the other.
    KindsDiffer { before: &'static str, after: &'static str },
}

impl std::fmt::Display for DiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiffError::KindsDiffer { before, after } => write!(
                f,
                "a {before} diagram cannot be compared with a {after} diagram: draw each on its own"
            ),
        }
    }
}

impl std::error::Error for DiffError {}

/// The document of two versions of a diagram. Still, it is the second
/// version with what was added, removed and changed marked. Animated, it
/// moves from the first version to the second.
pub fn diff_document(before: &Diagram, after: &Diagram, animated: bool) -> Result<Document, DiffError> {
    if before.kind() != after.kind() {
        return Err(DiffError::KindsDiffer {
            before: before.kind().word(),
            after: after.kind().word(),
        });
    }
    let diff = diff::diff(&layout(before), &layout(after));
    Ok(paint::diff_document(&diff, animated))
}
