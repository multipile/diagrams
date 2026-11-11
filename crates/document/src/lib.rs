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
//!   box cli pile="@cli"
//!   box storage pile="@storage"
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
