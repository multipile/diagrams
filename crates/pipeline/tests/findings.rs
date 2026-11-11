//! A file that breaks a rule is refused, with what is wrong at the line and
//! column that wrote it. Each case in `cases/invalid/` says in its first
//! line the finding it expects: `// expect: <code> at <line>:<column>`.

mod common;

use common::*;
use diagram_pipeline::{Error, StillFormat};

fn expected(case: &Case) -> (String, usize, usize) {
    let first = case.source.lines().next().unwrap_or_default();
    let rest = first
        .strip_prefix("// expect: ")
        .unwrap_or_else(|| panic!("{}: the first line says the finding it expects", case.name));
    let (code, at) = rest.split_once(" at ").expect("<code> at <line>:<column>");
    let (line, column) = at.split_once(':').expect("<line>:<column>");
    (code.to_string(), line.parse().unwrap(), column.parse().unwrap())
}

#[test]
fn every_broken_file_is_refused_with_the_finding_it_breaks() {
    let cases = cases("invalid");
    assert!(cases.len() >= 20, "cases/invalid has {}", cases.len());
    for case in cases {
        let (code, line, column) = expected(&case);
        let findings = match diagram_pipeline::read(&case.source) {
            Ok(_) => panic!("{}: read, and it breaks {code}", case.name),
            Err(Error::Read(findings)) => findings,
            Err(other) => panic!("{}: {other}", case.name),
        };
        let found = findings
            .0
            .iter()
            .find(|finding| finding.code == code)
            .unwrap_or_else(|| {
                panic!("{}: no {code} among:\n{findings}", case.name);
            });
        assert_eq!((found.line, found.column), (line, column), "{}: {found}", case.name);
    }
}

#[test]
fn every_finding_says_what_fixes_it() {
    for case in cases("invalid") {
        let Err(Error::Read(findings)) = diagram_pipeline::read(&case.source) else {
            panic!("{} reads", case.name);
        };
        for finding in &findings.0 {
            assert!(
                finding.help.is_some(),
                "{}: {finding} says nothing of how to fix it",
                case.name
            );
            assert!(!finding.message.is_empty());
        }
    }
}

#[test]
fn a_mistyped_word_is_answered_with_the_word_it_is_near() {
    for (name, near) in [
        ("type-unknown", "did you mean dependency?"),
        ("node-unknown", "did you mean box?"),
        ("word-unknown", "did you mean store?"),
        ("key-unknown", "did you mean storage?"),
    ] {
        let case = cases("invalid").into_iter().find(|c| c.name.ends_with(name)).unwrap();
        let Err(Error::Read(findings)) = diagram_pipeline::read(&case.source) else {
            panic!("{name} reads");
        };
        assert!(
            findings.0.iter().any(|f| f.help.as_deref() == Some(near)),
            "{name}: {findings}"
        );
    }
}

#[test]
fn every_mistake_in_a_file_is_found_in_one_read() {
    let source = r#"
diagram dependency version=1 {
  title "Three mistakes"
  desc "Each is found at once."
  box a as=stroe
  bxo b
  box c colour="red"
}
"#;
    let Err(Error::Read(findings)) = diagram_pipeline::read(source) else {
        panic!("it reads");
    };
    assert_eq!(
        findings.codes(),
        vec!["word-unknown", "node-unknown", "property-unknown"]
    );
    assert_eq!(findings.0.iter().map(|f| f.line).collect::<Vec<_>>(), vec![5, 6, 7]);
}

#[test]
fn a_refused_file_is_never_drawn() {
    let renderer = renderer();
    for case in cases("invalid") {
        assert!(matches!(
            diagram_pipeline::still(&renderer, &case.source, StillFormat::Png),
            Err(Error::Read(_))
        ));
        assert!(matches!(
            diagram_pipeline::page(&case.source, diagram_pipeline::Animation::None),
            Err(Error::Read(_))
        ));
    }
}
