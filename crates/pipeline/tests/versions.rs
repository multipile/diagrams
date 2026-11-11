//! A diagram file says the version of the format it is written in, and
//! every version the reader knows stays readable.

use diagram_pipeline::{Error, format::Format};

const V1: &str = r#"diagram tree version=1 {
  title "A tree"
  desc "One root."
  item "root"
}"#;

#[test]
fn a_file_is_read_by_the_version_it_says() {
    let read = diagram_pipeline::read(V1).unwrap();
    assert_eq!(read.format, Format::Kdl { version: 1 });
}

#[test]
fn every_format_the_reader_knows_is_listed_and_the_latest_is_the_last() {
    let all = Format::all();
    assert!(all.contains(&Format::Kdl { version: 1 }));
    assert_eq!(Format::latest(), *all.last().unwrap());
}

#[test]
fn a_file_that_says_no_version_is_refused_and_told_the_latest() {
    let source = V1.replace(" version=1", "");
    let Err(Error::Read(findings)) = diagram_pipeline::read(&source) else {
        panic!("it reads");
    };
    let Format::Kdl { version } = Format::latest();
    assert_eq!(findings.codes(), vec!["version-missing"]);
    assert_eq!(
        findings.0[0].help.as_deref(),
        Some(format!("add version={version} to the diagram node").as_str())
    );
}

#[test]
fn a_version_the_reader_does_not_know_is_refused_and_not_guessed() {
    for version in ["0", "2", "99"] {
        let source = V1.replace("version=1", &format!("version={version}"));
        let Err(Error::Read(findings)) = diagram_pipeline::read(&source) else {
            panic!("version {version} reads");
        };
        assert_eq!(findings.codes(), vec!["version-unknown"], "version {version}");
    }
    let source = V1.replace("version=1", "version=\"1\"");
    let Err(Error::Read(findings)) = diagram_pipeline::read(&source) else {
        panic!("a quoted version reads");
    };
    assert_eq!(findings.codes(), vec!["version-not-a-number"]);
}

#[test]
fn the_version_is_read_before_anything_else_in_the_file() {
    // A newer version may have words this reader does not know: they are
    // not reported, only the version is.
    let source = r#"diagram tree version=2 {
  title "A tree from a newer format"
  desc "With a node version 1 does not have."
  branch "root"
}"#;
    let Err(Error::Read(findings)) = diagram_pipeline::read(source) else {
        panic!("it reads");
    };
    assert_eq!(findings.codes(), vec!["version-unknown"]);
}
