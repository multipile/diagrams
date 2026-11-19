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

pub use diff::{Diff, Report, Subject};
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

fn same_kind(before: &Diagram, after: &Diagram) -> Result<(), DiffError> {
    if before.kind() == after.kind() {
        return Ok(());
    }
    Err(DiffError::KindsDiffer {
        before: before.kind().word(),
        after: after.kind().word(),
    })
}

/// What is drawn differently between two versions of a diagram, for its
/// caller to decide with: each element by its key, each line by the keys
/// of its two ends, and what the file names by no key by its id, as
/// [`Subject`] says. An element is added when the second version alone
/// has it, removed when the first alone has it, and changed when both have
/// it and draw it differently: another name, look or label. Where it is
/// drawn is no difference, and neither is its metadata, which is no part
/// of the comparison.
///
/// The report decides nothing. A caller may pass it to [`diff_document`]
/// as it is, or change it first: add what it holds changed though it is
/// drawn the same, or take out what it holds unchanged.
///
/// ```
/// use diagram_document::Subject;
/// let read = |boxes: &str| {
///     let source = format!("diagram dependency version=1 {{\n title \"t\"\n desc \"d\"\n{boxes}\n}}");
///     diagram_format::read(&source).unwrap().diagram
/// };
/// let before = read("box cli\nbox storage\nline cli storage");
/// let after = read("box cli \"the cli\"\nbox storage\nbox docs\nline cli storage");
/// let mut report = diagram_document::compare(&before, &after).unwrap();
/// assert_eq!(report.added, vec![Subject::element("docs")]);
/// assert_eq!(report.changed, vec![Subject::element("cli")]);
/// // The caller holds that the line changed too, though it is drawn the same.
/// report.change(Subject::line("cli", "storage"));
/// let document = diagram_document::diff_document(&before, &after, &report, false).unwrap();
/// assert_eq!(document.changes.unwrap().changed, 2);
/// ```
pub fn compare(before: &Diagram, after: &Diagram) -> Result<Report, DiffError> {
    same_kind(before, after)?;
    Ok(diff::compare(&layout(before), &layout(after)))
}

/// The document of two versions of a diagram, which draws exactly what
/// `report` says. Still, it is the second version with what the report
/// lists as added, removed and changed marked, and listed under it.
/// Animated, it moves from the first version to the second, and then marks
/// the same.
///
/// It decides nothing. An element the report does not list is drawn as
/// unchanged, though the two versions draw it differently: still, it is
/// the second version's, with no mark. An element the report lists as
/// changed is marked, though both versions draw it the same. An element of
/// the first version alone is drawn, faded, only when the report lists it.
/// [`compare`] gives the report of what is drawn differently.
pub fn diff_document(
    before: &Diagram,
    after: &Diagram,
    report: &Report,
    animated: bool,
) -> Result<Document, DiffError> {
    same_kind(before, after)?;
    let diff = diff::diff(&layout(before), &layout(after), report);
    Ok(paint::diff_document(&diff, animated))
}

/// A diagram's document with what `report` says marked: the diagram
/// compared with itself, as [`diff_document`] draws it, still. Each element
/// and each line the report lists is outlined and badged, and listed under
/// the diagram.
///
/// This is how a review shows a diagram whose own file a change did not
/// touch, though it touched what the diagram shows. What makes an element
/// changed is its caller's to say: the diagram knows only its keys.
pub fn changed_document(diagram: &Diagram, report: &Report) -> Document {
    let scene = layout(diagram);
    paint::diff_document(&diff::diff(&scene, &scene, report), false)
}
