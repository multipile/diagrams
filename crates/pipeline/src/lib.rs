//! Takes a diagram from its file to a picture: the file is read
//! (`diagram-format`), laid out and painted into a document
//! (`diagram-document`), and the document is written as a page
//! (`diagram-html`) or rendered as a picture (`diagram-render`). Both
//! outputs are written from the same document, so they show the same thing.
//!
//! The end-to-end tests of this repository live with this crate, in
//! `tests/`, and the diagrams they run are in `cases/`.

use std::fmt;

pub use diagram_document::{self as document, Animation, Document};
pub use diagram_format::{self as format, Diagram, Findings, Read};
pub use diagram_html::{self as html, Options};
pub use diagram_render::{self as render, MotionFormat, Renderer, StillFormat};

#[derive(Debug)]
pub enum Error {
    /// The file breaks a rule, or is not a diagram file.
    Read(Findings),
    /// Two diagrams that cannot be compared.
    Diff(diagram_document::DiffError),
    Render(diagram_render::RenderError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Read(findings) => write!(f, "{findings}"),
            Error::Diff(error) => write!(f, "{error}"),
            Error::Render(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<Findings> for Error {
    fn from(findings: Findings) -> Error {
        Error::Read(findings)
    }
}

impl From<diagram_document::DiffError> for Error {
    fn from(error: diagram_document::DiffError) -> Error {
        Error::Diff(error)
    }
}

impl From<diagram_render::RenderError> for Error {
    fn from(error: diagram_render::RenderError) -> Error {
        Error::Render(error)
    }
}

/// Reads a diagram file's text.
pub fn read(source: &str) -> Result<Read, Error> {
    Ok(diagram_format::read(source)?)
}

/// A diagram file's document, still or revealed.
pub fn document(source: &str, animation: Animation) -> Result<Document, Error> {
    Ok(diagram_document::document(&read(source)?.diagram, animation))
}

/// The document of two versions of a diagram file: still, the second
/// version with its changes marked; animated, the first becoming the
/// second.
pub fn diff_document(before: &str, after: &str, animated: bool) -> Result<Document, Error> {
    let (before, after) = (read(before)?, read(after)?);
    Ok(diagram_document::diff_document(
        &before.diagram,
        &after.diagram,
        animated,
    )?)
}

/// A diagram file's page, still or revealed, with its fonts embedded.
pub fn page(source: &str, animation: Animation) -> Result<String, Error> {
    Ok(diagram_html::page(&document(source, animation)?, &Options::default()))
}

/// A diagram file as a still picture: the finished diagram.
pub fn still(renderer: &Renderer, source: &str, format: StillFormat) -> Result<Vec<u8>, Error> {
    Ok(renderer.still(&document(source, Animation::None)?, format, None)?)
}

/// A diagram file revealed, element by element, as an animation.
pub fn reveal(renderer: &Renderer, source: &str, format: MotionFormat, fps: u32) -> Result<Vec<u8>, Error> {
    Ok(renderer.animation(&document(source, Animation::Reveal)?, format, fps)?)
}

/// One moment of a diagram file's reveal, `at_ms` into it.
pub fn reveal_at(renderer: &Renderer, source: &str, format: StillFormat, at_ms: u32) -> Result<Vec<u8>, Error> {
    Ok(renderer.still(&document(source, Animation::Reveal)?, format, Some(at_ms))?)
}

/// The page of two versions of a diagram file, with its fonts embedded.
pub fn diff_page(before: &str, after: &str, animated: bool) -> Result<String, Error> {
    Ok(diagram_html::page(
        &diff_document(before, after, animated)?,
        &Options::default(),
    ))
}

/// Two versions of a diagram file as a still picture, their changes marked.
pub fn diff_still(renderer: &Renderer, before: &str, after: &str, format: StillFormat) -> Result<Vec<u8>, Error> {
    Ok(renderer.still(&diff_document(before, after, false)?, format, None)?)
}

/// Two versions of a diagram file as an animation, from the first to the
/// second.
pub fn diff_animation(
    renderer: &Renderer,
    before: &str,
    after: &str,
    format: MotionFormat,
    fps: u32,
) -> Result<Vec<u8>, Error> {
    Ok(renderer.animation(&diff_document(before, after, true)?, format, fps)?)
}

/// One moment of the animation from one version of a diagram file to the
/// next, `at_ms` into it.
pub fn diff_at(
    renderer: &Renderer,
    before: &str,
    after: &str,
    format: StillFormat,
    at_ms: u32,
) -> Result<Vec<u8>, Error> {
    Ok(renderer.still(&diff_document(before, after, true)?, format, Some(at_ms))?)
}
