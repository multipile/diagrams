//! What is wrong with a file, where, and what fixes it.

use std::fmt;

use crate::model::Span;

/// One thing wrong with a file. Its code is stable, so a test or a tool can
/// match it; its message and help are for whoever fixes the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub code: &'static str,
    pub message: String,
    pub help: Option<String>,
    pub span: Span,
    /// Counted from 1.
    pub line: usize,
    /// Counted from 1, in characters.
    pub column: usize,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}: {}", self.line, self.column, self.code, self.message)?;
        if let Some(help) = &self.help {
            write!(f, " ({help})")?;
        }
        Ok(())
    }
}

/// Every finding of one read, in the order of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Findings(pub Vec<Finding>);

impl Findings {
    pub fn codes(&self) -> Vec<&'static str> {
        self.0.iter().map(|finding| finding.code).collect()
    }
}

impl fmt::Display for Findings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, finding) in self.0.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{finding}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Findings {}

/// Gathers findings while a file is read, and gives them a line and a column
/// once it is done.
#[derive(Debug, Default)]
pub(crate) struct Report {
    found: Vec<(&'static str, String, Option<String>, Span)>,
}

impl Report {
    pub fn add(&mut self, code: &'static str, span: Span, message: impl Into<String>) {
        self.found.push((code, message.into(), None, span));
    }

    pub fn help(&mut self, code: &'static str, span: Span, message: impl Into<String>, help: impl Into<String>) {
        self.found.push((code, message.into(), Some(help.into()), span));
    }

    pub fn is_empty(&self) -> bool {
        self.found.is_empty()
    }

    pub fn finish(self, source: &str) -> Findings {
        let mut findings: Vec<Finding> = self
            .found
            .into_iter()
            .map(|(code, message, help, span)| {
                let (line, column) = locate(source, span.offset);
                Finding {
                    code,
                    message,
                    help,
                    span,
                    line,
                    column,
                }
            })
            .collect();
        findings.sort_by_key(|finding| (finding.span.offset, finding.code));
        findings.dedup();
        Findings(findings)
    }
}

/// The line and column, from 1, of a byte offset into `source`.
pub fn locate(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..floor_char_boundary(source, offset)];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |text| text.chars().count()) + 1;
    (line, column)
}

fn floor_char_boundary(text: &str, mut offset: usize) -> usize {
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// The word of `known` nearest to `word`, when it is near enough to be what
/// was meant.
pub fn nearest<'a>(word: &str, known: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    known
        .into_iter()
        .map(|candidate| (distance(word, candidate), candidate))
        .filter(|(distance, candidate)| {
            *distance <= 2.max(candidate.chars().count() / 3) && *distance < word.chars().count().max(1)
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate)
}

/// "did you mean x?", or nothing.
pub fn did_you_mean<'a>(word: &str, known: impl IntoIterator<Item = &'a str>) -> Option<String> {
    nearest(word, known).map(|candidate| format!("did you mean {candidate}?"))
}

/// The edit distance between two words, in characters.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current.push((previous[j] + cost).min(previous[j + 1] + 1).min(current[j] + 1));
        }
        previous = current;
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_location_counts_lines_and_characters_from_one() {
        let source = "ab\ncdé\nf";
        assert_eq!(locate(source, 0), (1, 1));
        assert_eq!(locate(source, 3), (2, 1));
        assert_eq!(locate(source, 7), (2, 4));
        assert_eq!(locate(source, source.len()), (3, 2));
    }

    #[test]
    fn a_mistyped_word_finds_the_word_it_is_near() {
        assert_eq!(nearest("stroage", ["storage", "piles", "cli"]), Some("storage"));
        assert_eq!(nearest("bxo", ["box", "line"]), Some("box"));
        assert_eq!(nearest("zzzzzz", ["box", "line"]), None);
    }
}
