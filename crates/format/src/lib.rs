//! Reads multipile's diagram files into a [`Diagram`].
//!
//! A diagram file says what a diagram shows, and never where: its type, its
//! boxes and lines, which element is focal. Today every diagram file is a
//! `.diagram` file written in KDL ([`Format::Kdl`]). The file says the
//! version of the format it is written in, and every version stays
//! readable, so a file never has to be rewritten because the format moved.
//! Another format, or another version, is read into the same [`Diagram`].
//!
//! Reading checks the rules every diagram is held to, and its type's limits.
//! A file that breaks any of them is refused with every [`Finding`], each at
//! its line and column, and with what fixes it.
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
//! assert_eq!(read.diagram.kind(), diagram_format::Kind::Dependency);
//! assert_eq!(read.format, diagram_format::Format::Kdl { version: 1 });
//! ```

pub mod finding;
mod kdl;
pub mod model;
pub mod rules;

pub use finding::{Finding, Findings};
pub use model::*;

/// The file format a diagram was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A `.diagram` file in KDL, at a version of the format.
    Kdl { version: u32 },
}

impl Format {
    /// Every format and version this crate reads, oldest first.
    pub fn all() -> Vec<Format> {
        kdl::VERSIONS.iter().map(|&version| Format::Kdl { version }).collect()
    }

    /// The format a new file is written in.
    pub fn latest() -> Format {
        *Self::all().last().expect("at least one format")
    }
}

/// A diagram, and the format its file was written in.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    pub diagram: Diagram,
    pub format: Format,
}

/// Reads a `.diagram` file's text.
pub fn read(source: &str) -> Result<Read, Findings> {
    let mut report = finding::Report::default();
    let read = kdl::read(source, &mut report);
    if let Some((diagram, _)) = &read {
        rules::check(diagram, &mut report);
    }
    match read {
        Some((diagram, version)) if report.is_empty() => Ok(Read {
            diagram,
            format: Format::Kdl { version },
        }),
        _ => Err(report.finish(source)),
    }
}
