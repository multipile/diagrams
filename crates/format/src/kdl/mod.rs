//! `.diagram` files written in KDL.
//!
//! A file holds one `diagram` node, which names the diagram's type and the
//! version of the format it is written in:
//!
//! ```kdl
//! diagram dependency version=1 {
//!   title "How the crates depend on one another"
//!   …
//! }
//! ```
//!
//! The version is read first, and the rest of the file is read by the
//! reader of that version. Every version stays readable, and each is read
//! into the same [`Diagram`](crate::Diagram), so a version added later
//! changes nothing for a file written in an earlier one.

mod v1;

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

use crate::finding::{Report, did_you_mean};
use crate::model::{Diagram, Kind, Span};

/// The versions of the KDL format this reader reads, oldest first.
pub const VERSIONS: &[u32] = &[1];

pub(crate) fn read(source: &str, report: &mut Report) -> Option<(Diagram, u32)> {
    let document: KdlDocument = match source.parse() {
        Ok(document) => document,
        Err(error) => {
            let error: kdl::KdlError = error;
            for diagnostic in error.diagnostics {
                let span = Span {
                    offset: diagnostic.span.offset(),
                    len: diagnostic.span.len(),
                };
                let message = diagnostic
                    .message
                    .or(diagnostic.label)
                    .unwrap_or_else(|| "this is not KDL".to_string());
                match diagnostic.help {
                    Some(help) => report.help("kdl-syntax", span, message, help),
                    None => report.add("kdl-syntax", span, message),
                }
            }
            return None;
        }
    };

    let mut diagram = None;
    for node in document.nodes() {
        let name = node.name().value();
        if name != "diagram" {
            report.help(
                "not-a-diagram",
                span(node),
                format!("a .diagram file holds one `diagram` node, not `{name}`"),
                "put everything inside the diagram node",
            );
        } else if diagram.is_some() {
            report.help(
                "two-diagrams",
                span(node),
                "a .diagram file holds one diagram",
                "put the second diagram in a file of its own",
            );
        } else {
            diagram = Some(node);
        }
    }
    let Some(node) = diagram else {
        if report.is_empty() {
            report.help(
                "no-diagram",
                Span::default(),
                "the file holds no diagram",
                "start it with: diagram dependency version=1 { … }",
            );
        }
        return None;
    };

    let version = version(node, report)?;
    let kind = kind(node, report)?;
    if !report.is_empty() {
        return None;
    }
    let diagram = match version {
        1 => v1::read(node, kind, report),
        _ => unreachable!("version checks the versions it knows"),
    };
    if report.is_empty() {
        diagram.map(|diagram| (diagram, version))
    } else {
        None
    }
}

fn version(node: &KdlNode, report: &mut Report) -> Option<u32> {
    let Some(entry) = Reader::prop_entry(node, "version") else {
        report.help(
            "version-missing",
            span(node),
            "the diagram does not say which version of the format it is written in",
            format!("add version={} to the diagram node", VERSIONS[VERSIONS.len() - 1]),
        );
        return None;
    };
    let known = VERSIONS.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
    match entry.value() {
        KdlValue::Integer(version) if u32::try_from(*version).is_ok_and(|v| VERSIONS.contains(&v)) => {
            Some(*version as u32)
        }
        KdlValue::Integer(version) => {
            report.help(
                "version-unknown",
                entry_span(entry),
                format!("this reader does not know version {version} of the .diagram format"),
                format!("it reads versions {known}: write one of those, or read the file with a newer reader"),
            );
            None
        }
        other => {
            report.help(
                "version-not-a-number",
                entry_span(entry),
                format!("the version is a number, not {other}"),
                format!("write version={}", VERSIONS[VERSIONS.len() - 1]),
            );
            None
        }
    }
}

fn kind(node: &KdlNode, report: &mut Report) -> Option<Kind> {
    let words: Vec<&str> = Kind::ALL.iter().map(|kind| kind.word()).collect();
    let Some(entry) = node.entries().iter().find(|entry| entry.name().is_none()) else {
        report.help(
            "type-missing",
            span(node),
            "the diagram does not say its type",
            format!("name one after `diagram`: {}", words.join(", ")),
        );
        return None;
    };
    let word = match entry.value() {
        KdlValue::String(word) => word.as_str(),
        _ => "",
    };
    match Kind::from_word(word) {
        Some(kind) => Some(kind),
        None => {
            let help = did_you_mean(word, words.iter().copied())
                .unwrap_or_else(|| format!("the types are {}", words.join(", ")));
            report.help(
                "type-unknown",
                entry_span(entry),
                format!("there is no diagram type {}", entry.value()),
                help,
            );
            None
        }
    }
}

pub(crate) fn span(node: &KdlNode) -> Span {
    let span = node.span();
    Span {
        offset: span.offset(),
        len: span.len(),
    }
}

pub(crate) fn entry_span(entry: &KdlEntry) -> Span {
    let span = entry.span();
    Span {
        offset: span.offset(),
        len: span.len(),
    }
}

/// Reads the arguments, properties and children of nodes, and reports what
/// a node holds that it may not.
pub(crate) struct Reader<'r> {
    pub report: &'r mut Report,
}

/// What a node may hold, for [`Reader::shape`].
pub(crate) struct Shape<'s> {
    /// The node as it is written, for the help of a finding.
    pub usage: &'s str,
    /// How many arguments it must have, and may have.
    pub args: (usize, usize),
    pub props: &'s [&'s str],
    pub children: bool,
}

impl Reader<'_> {
    /// Whether the node holds what `shape` says, reporting what it does not.
    pub fn shape(&mut self, node: &KdlNode, shape: &Shape) -> bool {
        let name = node.name().value();
        let mut fine = true;
        let args: Vec<&KdlEntry> = Self::args(node);
        if args.len() < shape.args.0 {
            self.report.help(
                "argument-missing",
                span(node),
                format!("`{name}` needs {} more", shape.args.0 - args.len()),
                format!("write it as: {}", shape.usage),
            );
            fine = false;
        }
        for extra in args.iter().skip(shape.args.1) {
            self.report.help(
                "argument-extra",
                entry_span(extra),
                format!("`{name}` takes no argument {}", extra.value()),
                format!("write it as: {}", shape.usage),
            );
            fine = false;
        }
        let mut seen = Vec::new();
        for entry in node.entries() {
            let Some(prop) = entry.name() else { continue };
            let prop = prop.value();
            if !shape.props.contains(&prop) {
                let help = did_you_mean(prop, shape.props.iter().copied()).unwrap_or_else(|| {
                    if shape.props.is_empty() {
                        format!("`{name}` takes no property")
                    } else {
                        format!("`{name}` takes {}", shape.props.join(", "))
                    }
                });
                self.report.help(
                    "property-unknown",
                    entry_span(entry),
                    format!("`{name}` has no property {prop}"),
                    help,
                );
                fine = false;
            } else if seen.contains(&prop) {
                self.report
                    .add("property-repeated", entry_span(entry), format!("{prop} is given twice"));
                fine = false;
            }
            seen.push(prop);
        }
        if !shape.children && node.children().is_some_and(|children| !children.nodes().is_empty()) {
            self.report.help(
                "children-not-allowed",
                span(node),
                format!("`{name}` holds nothing"),
                format!("write it as: {}", shape.usage),
            );
            fine = false;
        }
        fine
    }

    pub fn args(node: &KdlNode) -> Vec<&KdlEntry> {
        node.entries().iter().filter(|entry| entry.name().is_none()).collect()
    }

    pub fn prop_entry<'n>(node: &'n KdlNode, name: &str) -> Option<&'n KdlEntry> {
        node.entries()
            .iter()
            .rev()
            .find(|entry| entry.name().is_some_and(|prop| prop.value() == name))
    }

    pub fn children(node: &KdlNode) -> &[KdlNode] {
        node.children().map_or(&[], |children| children.nodes())
    }

    /// The text of an entry. A number is not text.
    pub fn text(&mut self, entry: &KdlEntry, what: &str) -> Option<String> {
        match entry.value() {
            KdlValue::String(text) => Some(text.clone()),
            other => {
                self.report.help(
                    "not-text",
                    entry_span(entry),
                    format!("{what} is text, not {other}"),
                    "put it in double quotes",
                );
                None
            }
        }
    }

    pub fn text_arg(&mut self, node: &KdlNode, index: usize, what: &str) -> Option<String> {
        let entry = *Self::args(node).get(index)?;
        self.text(entry, what)
    }

    /// A key: letters, digits, `_` and `-`, starting with a letter.
    pub fn key_arg(&mut self, node: &KdlNode, index: usize) -> Option<(String, Span)> {
        let entry = *Self::args(node).get(index)?;
        let key = self.text(entry, "a key")?;
        let mut chars = key.chars();
        let fine = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !fine {
            self.report.help(
                "key-invalid",
                entry_span(entry),
                format!("{key:?} is not a key"),
                "a key is letters, digits, _ and -, and starts with a letter",
            );
            return None;
        }
        Some((key, entry_span(entry)))
    }

    pub fn text_prop(&mut self, node: &KdlNode, name: &str) -> Option<String> {
        let entry = Self::prop_entry(node, name)?;
        self.text(entry, name)
    }

    pub fn bool_prop(&mut self, node: &KdlNode, name: &str) -> bool {
        let Some(entry) = Self::prop_entry(node, name) else {
            return false;
        };
        match entry.value() {
            KdlValue::Bool(value) => *value,
            other => {
                self.report.help(
                    "not-a-boolean",
                    entry_span(entry),
                    format!("{name} is #true or #false, not {other}"),
                    format!("write {name}=#true"),
                );
                false
            }
        }
    }

    pub fn number_prop(&mut self, node: &KdlNode, name: &str) -> Option<f64> {
        let entry = Self::prop_entry(node, name)?;
        match entry.value() {
            KdlValue::Integer(value) => Some(*value as f64),
            KdlValue::Float(value) => Some(*value),
            other => {
                self.report.help(
                    "not-a-number",
                    entry_span(entry),
                    format!("{name} is a number, not {other}"),
                    "write it without quotes: 1200 or 3.5",
                );
                None
            }
        }
    }

    /// One word of a few.
    pub fn word_prop<'w>(&mut self, node: &KdlNode, name: &str, words: &[&'w str]) -> Option<&'w str> {
        let entry = Self::prop_entry(node, name)?;
        let given = self.text(entry, name)?;
        match words.iter().find(|word| **word == given) {
            Some(word) => Some(word),
            None => {
                let help = did_you_mean(&given, words.iter().copied())
                    .unwrap_or_else(|| format!("{name} is one of {}", words.join(", ")));
                self.report.help(
                    "word-unknown",
                    entry_span(entry),
                    format!("{name} cannot be {given}"),
                    help,
                );
                None
            }
        }
    }

    /// A child node that its parent does not take.
    pub fn unknown(&mut self, node: &KdlNode, parent: &str, known: &[&str]) {
        let name = node.name().value();
        let help = did_you_mean(name, known.iter().copied())
            .unwrap_or_else(|| format!("`{parent}` holds {}", known.join(", ")));
        self.report.help(
            "node-unknown",
            span(node),
            format!("`{parent}` holds no `{name}`"),
            help,
        );
    }
}
