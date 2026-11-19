//! A diagram carries metadata it never reads: properties written
//! `<owner>:<key>` on an element or a line, and `meta <owner> { … }` blocks
//! for the whole diagram. Each is kept as it is written, with where it is
//! written, and a page carries an element's and a line's with its node.

mod common;

use common::*;
use diagram_pipeline::format::{Body, Branch, Diagram, Item, Line, Meta, MetaNode, MetaValue, Text};
use diagram_pipeline::{Animation, Error};

/// Every element a diagram names, whatever its type.
fn items(diagram: &Diagram) -> Vec<&Item> {
    match &diagram.body {
        Body::Architecture(body) => body
            .boxes
            .iter()
            .flat_map(|held| std::iter::once(&held.item).chain(&held.holds))
            .collect(),
        Body::Sequence(body) => body.actors.iter().collect(),
        Body::State(body) => body.states.iter().map(|state| &state.item).collect(),
        Body::Swimlane(body) => body.lanes.iter().flat_map(|lane| &lane.steps).collect(),
        Body::Process(body) => body
            .lanes
            .iter()
            .flat_map(|lane| &lane.tasks)
            .map(|task| &task.item)
            .collect(),
        Body::Tree(body) => body
            .root
            .walk()
            .into_iter()
            .map(|branch: &Branch| &branch.item)
            .collect(),
        Body::Dependency(body) => body.boxes.iter().collect(),
        Body::Nested(body) => body.levels.iter().collect(),
        Body::Treemap(body) => body.parts.iter().map(|part| &part.item).collect(),
    }
}

/// Every line that may carry metadata.
fn lines(diagram: &Diagram) -> Vec<&Line> {
    match &diagram.body {
        Body::Architecture(body) => body.lines.iter().collect(),
        Body::Swimlane(body) => body.lines.iter().collect(),
        Body::Dependency(body) => body.lines.iter().chain(&body.cycle).collect(),
        _ => Vec::new(),
    }
}

fn case(name: &str) -> Case {
    let (dir, _) = name.split_once('/').unwrap();
    cases(dir)
        .into_iter()
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("no case {name}"))
}

fn read(source: &str) -> Diagram {
    diagram_pipeline::read(source)
        .unwrap_or_else(|error| panic!("{error}"))
        .diagram
}

fn codes(source: &str) -> Vec<&'static str> {
    match diagram_pipeline::read(source) {
        Err(Error::Read(findings)) => findings.codes(),
        Ok(_) => Vec::new(),
        Err(other) => panic!("{other}"),
    }
}

fn written(meta: &[Meta]) -> Vec<String> {
    meta.iter()
        .map(|meta| format!("{}:{}={}", meta.owner, meta.key, meta.value))
        .collect()
}

/// A dependency diagram of one box, written as `first`, and whatever else
/// `rest` says.
fn boxed(first: &str, rest: &str) -> String {
    format!("diagram dependency version=1 {{\n  title \"One box\"\n  desc \"A box.\"\n  {first}\n{rest}}}\n")
}

#[test]
fn every_type_keeps_the_metadata_of_its_elements_where_it_is_written() {
    for kind in TYPES {
        let case = case(&format!("{kind}/its-elements-carry-metadata"));
        let diagram = case.diagram();
        let items = items(&diagram);
        assert!(
            items.iter().all(|item| !item.meta.is_empty()),
            "{}: every element of the case carries some",
            case.name
        );
        for meta in items
            .iter()
            .flat_map(|item| &item.meta)
            .chain(lines(&diagram).into_iter().flat_map(|line| &line.meta))
        {
            let source = &case.source[meta.span.offset..meta.span.offset + meta.span.len];
            assert!(
                source.starts_with(&format!("{}:{}=", meta.owner, meta.key)),
                "{}: {source}",
                case.name
            );
            if let MetaValue::Text(text) = &meta.value {
                assert!(
                    case.source[text.lines[0]..].starts_with(&text.text),
                    "{}: {source}",
                    case.name
                );
            }
        }
        assert!(diagram.meta.is_empty(), "{}", case.name);
    }
}

#[test]
fn a_line_keeps_its_metadata_in_the_three_types_that_write_lines() {
    for (kind, expected) in [
        ("architecture", 3),
        ("swimlane", 3),
        // Four lines and the cycle.
        ("dependency", 5),
    ] {
        let case = case(&format!("{kind}/its-elements-carry-metadata"));
        let diagram = case.diagram();
        let count = lines(&diagram).iter().map(|line| line.meta.len()).sum::<usize>();
        assert_eq!(count, expected, "{}", case.name);
    }
    let diagram = case("dependency/its-elements-carry-metadata").diagram();
    let Body::Dependency(body) = &diagram.body else {
        panic!()
    };
    assert_eq!(
        written(&body.cycle.as_ref().unwrap().meta),
        vec!["pile:edge=calls-back"]
    );
    assert!(body.lines[4].meta.is_empty(), "a line with none has none");
}

#[test]
fn a_value_is_text_a_number_or_a_boolean() {
    let diagram = case("architecture/its-elements-carry-metadata").diagram();
    let all: Vec<String> = items(&diagram)
        .into_iter()
        .flat_map(|item| written(&item.meta))
        .collect();
    assert_eq!(
        all,
        vec![
            "team:owner=web",
            "team:on-call=false",
            "team:owner=orders",
            "team:tier=1",
            "team:owner=payments",
            "team:owner=platform",
            "team:tier=1",
            "team:owner=growth",
            "team:uptime=99.5",
        ]
    );
    let Body::Architecture(body) = &diagram.body else {
        panic!()
    };
    assert_eq!(body.boxes[0].item.meta[1].value, MetaValue::Bool(false));
    assert_eq!(body.boxes[1].item.meta[1].value, MetaValue::Integer(1));
    assert_eq!(body.boxes[3].item.meta[1].value, MetaValue::Float(99.5));
    assert_eq!(body.lines[0].meta[1].value, MetaValue::Integer(2));
    // A word needs no quotes, and is text all the same.
    let diagram = case("dependency/its-elements-carry-metadata").diagram();
    let Body::Dependency(body) = &diagram.body else {
        panic!()
    };
    assert_eq!(body.boxes[4].meta[1].value.as_text().unwrap().text, "external");
}

#[test]
fn every_type_keeps_a_metadata_block_and_draws_none_of_it() {
    for kind in TYPES {
        let case = case(&format!("{kind}/it-carries-a-metadata-block"));
        let diagram = case.diagram();
        assert!(!diagram.meta.is_empty(), "{}", case.name);
        for block in &diagram.meta {
            assert!(
                case.source[block.span.offset..].starts_with(&format!("meta {} {{", block.owner)),
                "{}",
                case.name
            );
            assert!(!block.nodes.is_empty(), "{}", case.name);
        }
        // The same diagram with its blocks taken out draws the same page.
        let mut bare = case.source.clone();
        for block in diagram.meta.iter().rev() {
            bare.replace_range(block.span.offset..block.span.offset + block.span.len, "");
        }
        assert!(read(&bare).meta.is_empty());
        assert_eq!(
            diagram_pipeline::page(&case.source, Animation::Reveal).unwrap(),
            diagram_pipeline::page(&bare, Animation::Reveal).unwrap(),
            "{}",
            case.name
        );
    }
}

#[test]
fn a_block_keeps_its_nodes_as_a_tree_with_arguments_properties_and_children() {
    let case = case("architecture/it-carries-a-metadata-block");
    let diagram = case.diagram();
    let [block] = diagram.meta.as_slice() else { panic!() };
    assert_eq!(block.owner, "review");
    let names: Vec<&str> = block.nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(names, vec!["last", "every", "covers"]);
    let last = &block.nodes[0];
    assert_eq!(last.args[0].0.to_string(), "2026-03-02");
    assert_eq!(last.props[0].0, "by");
    assert_eq!(last.props[0].1.to_string(), "the platform team");
    let span = last.props[0].2;
    assert_eq!(
        &case.source[span.offset..span.offset + span.len],
        "by=\"the platform team\""
    );
    assert_eq!(block.nodes[1].props[0].1, MetaValue::Integer(12));
    let covers = &block.nodes[2];
    assert!(covers.args.is_empty() && covers.props.is_empty());
    assert_eq!(covers.children.len(), 2);
    assert_eq!(covers.children[0].props[0].1, MetaValue::Bool(true));
    assert!(case.source[covers.children[1].span.offset..].starts_with("service \"storefront\""));
}

/// Every string in a block's nodes, depth first.
fn texts(nodes: &[MetaNode]) -> Vec<&Text> {
    let mut all = Vec::new();
    for node in nodes {
        all.extend(node.args.iter().filter_map(|(value, _)| value.as_text()));
        all.extend(node.props.iter().filter_map(|(_, value, _)| value.as_text()));
        all.extend(texts(&node.children));
    }
    all
}

#[test]
fn a_string_says_where_each_of_its_lines_starts_in_the_file() {
    let case = case("dependency/it-carries-a-metadata-block");
    let diagram = case.diagram();
    assert_eq!(
        diagram
            .meta
            .iter()
            .map(|block| block.owner.as_str())
            .collect::<Vec<_>>(),
        vec!["checks", "docs"]
    );
    let boxes = &diagram.meta[0].nodes[0];
    // A raw string holds quotes as they are.
    let one = boxes.children[0].args[0].0.as_text().unwrap();
    assert_eq!(one.text, r#"?- depends(Crate, "model")."#);
    assert_eq!(one.lines.len(), 1);
    // Several arguments over continued lines, each a string of its own.
    let several = &boxes.children[1].args;
    assert_eq!(several.len(), 3);
    assert_eq!(several[2].0.to_string(), "?- near(Crate).");
    // A multi-line string loses its indentation, and still says where
    // each line is.
    let lines = &diagram.meta[0].nodes[1];
    let program = lines.children[0].args[0].0.as_text().unwrap();
    assert_eq!(
        program.text,
        "direct(A, B) :- depends(A, B), !through(A, B).\n?- direct(From, To)."
    );
    assert_eq!(program.lines.len(), 2);
    // Every character of every string is in the file where its line says.
    let all = texts(&diagram.meta[0].nodes);
    assert_eq!(all.len(), 7);
    for text in all {
        assert_eq!(text.lines.len(), text.text.split('\n').count());
        for (n, line) in text.text.split('\n').enumerate() {
            let start = text.lines[n];
            assert_eq!(&case.source[start..start + line.len()], line);
            let column = line.find("depends").unwrap_or(0);
            assert_eq!(text.offset(n, column), Some(start + column));
        }
    }
    // The second line of the program is on line 25 of the file, and starts at column 9.
    let at = program.offset(1, 3).unwrap();
    assert_eq!(diagram_pipeline::format::finding::locate(&case.source, at), (25, 12));
    assert!(case.source[at..].starts_with("direct(From, To)."));
}

#[test]
fn a_string_with_blank_lines_and_escapes_still_has_a_start_for_every_line() {
    let source = boxed(
        "box a",
        "  meta notes {\n    raw #\"\"\"\n      one\n\n        three\n      \"\"\"#\n    plain \"a\\nbc\\td\\n\\u{65}f\"\n    many \"\"\"\n      x\\ny\n      z\n      \"\"\"\n    word bare\n  }\n",
    );
    let diagram = read(&source);
    let nodes = &diagram.meta[0].nodes;
    let raw = nodes[0].args[0].0.as_text().unwrap();
    assert_eq!(raw.text, "one\n\n  three");
    assert!(source[raw.lines[0]..].starts_with("one\n"));
    assert!(
        source[raw.lines[1]..].starts_with('\n'),
        "a blank line starts at its end"
    );
    assert!(source[raw.lines[2]..].starts_with("  three\n"));
    // An escaped newline starts a line after the escape.
    let plain = nodes[1].args[0].0.as_text().unwrap();
    assert_eq!(plain.text, "a\nbc\td\nef");
    assert_eq!(plain.lines.len(), 3);
    assert!(source[plain.lines[0]..].starts_with("a\\nbc"));
    assert!(source[plain.lines[1]..].starts_with("bc\\td"));
    assert!(source[plain.lines[2]..].starts_with("\\u{65}f\""));
    // Escapes that move the lines of a multi-line string: every line is
    // said to start where the string's first line does.
    let many = nodes[2].args[0].0.as_text().unwrap();
    assert_eq!(many.text, "x\ny\nz");
    assert_eq!(many.lines.len(), 3);
    assert!(many.lines.iter().all(|start| source[*start..].starts_with("x\\ny")));
    let word = nodes[3].args[0].0.as_text().unwrap();
    assert!(source[word.lines[0]..].starts_with("bare\n"));
    // On an element too.
    let source = boxed("box a note:text=#\"say \"hi\"\"# note:short=\"hi\"", "");
    let diagram = read(&source);
    let metas = &items(&diagram)[0].meta;
    let text = metas[0].value.as_text().unwrap();
    assert_eq!(text.text, "say \"hi\"");
    assert!(source[text.lines[0]..].starts_with("say \"hi\"\"#"));
    assert!(source[metas[1].value.as_text().unwrap().lines[0]..].starts_with("hi\"\n"));
}

#[test]
fn a_page_carries_the_metadata_of_each_element_and_line_with_its_node() {
    let case = case("dependency/its-elements-carry-metadata");
    let html = diagram_pipeline::page(&case.source, Animation::None).unwrap();
    assert!(html.contains("data-id=\"box:command\" data-pile-id=\"3f9c2b7e0a1d4c5e8f6a7b9c0d1e2f3a\""));
    assert!(html.contains(
        "data-id=\"box:serde\" data-pile-id=\"00112233445566778899aabbccddeeff\" data-pile-node-type=\"external\""
    ));
    assert!(html.contains("data-id=\"line:command-&gt;reader\" data-pile-edge=\"relies-on\""));
    assert!(html.contains("data-id=\"cycle:model-&gt;command\" data-pile-edge=\"calls-back\""));
    assert!(
        html.contains("data-id=\"line:model-&gt;serde\" style="),
        "a line with none has none"
    );
    // Numbers and booleans as text, and a level's with its region.
    let html = diagram_pipeline::page(&case_source("nested/its-elements-carry-metadata"), Animation::None).unwrap();
    assert!(html.contains("data-id=\"level:level-2\" data-policy-rule=\"mutual-tls\" data-policy-audited=\"true\""));
    assert!(html.contains("data-policy-rank=\"3\""));
    let html = diagram_pipeline::page(&case_source("treemap/its-elements-carry-metadata"), Animation::None).unwrap();
    assert!(html.contains("data-bill-growth=\"1.5\""));
    // A value is escaped as any attribute is.
    let source = boxed("box a note:text=#\"<b> & \"c\"\"#", "");
    let html = diagram_pipeline::page(&source, Animation::None).unwrap();
    assert!(html.contains("data-note-text=\"&lt;b&gt; &amp; &quot;c&quot;\""));
}

fn case_source(name: &str) -> String {
    case(name).source
}

#[test]
fn a_document_carries_each_elements_key_and_each_lines_two_keys() {
    let diagram = case("swimlane/its-elements-carry-metadata").diagram();
    let document = diagram_document::document(&diagram, Animation::None);
    let node = |id: &str| {
        *document
            .walk()
            .iter()
            .find(|node| node.id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("no {id}"))
    };
    let step = node("step:approve");
    assert_eq!(step.key.as_deref(), Some("approve"));
    assert_eq!(
        step.meta,
        vec![
            ("sla-hours".to_string(), "48".to_string()),
            ("sla-needs-two".to_string(), "true".to_string())
        ]
    );
    let line = node("line:pay->tell");
    assert_eq!(line.ends, Some(("pay".to_string(), "tell".to_string())));
    assert_eq!(line.key, None);
    assert_eq!(line.meta.len(), 2);
    assert_eq!(node("line:approve->pay").meta, vec![]);
    assert_eq!(node("heading").key, None);
    // A lane is not an element a file gives metadata, and has no key.
    assert_eq!(node("lane:finance").key, None);
    // A tree's item is named by its path, and a level by its place.
    let tree = diagram_document::document(&case("tree/its-elements-carry-metadata").diagram(), Animation::None);
    assert!(
        tree.walk()
            .iter()
            .any(|node| node.key.as_deref() == Some("home / guide / install"))
    );
    let nested = diagram_document::document(&case("nested/its-elements-carry-metadata").diagram(), Animation::None);
    assert!(nested.walk().iter().any(|node| node.key.as_deref() == Some("level-3")));
    // A transition is no `line`: it has no ends to be marked by.
    let state = diagram_document::document(&case("state/its-elements-carry-metadata").diagram(), Animation::None);
    assert!(state.walk().iter().all(|node| node.ends.is_none()));
}

#[test]
fn only_what_makes_metadata_unreadable_is_refused() {
    for (first, rest, expected) in [
        ("box a docs:page:anchor=\"x\"", "", "meta-invalid"),
        ("box a docs:=\"x\"", "", "meta-invalid"),
        ("box a :page=\"x\"", "", "meta-invalid"),
        ("box a \"docs page:x\"=1", "", "meta-invalid"),
        ("box a docs:page=#null", "", "meta-invalid"),
        ("box a docs:page=1 docs:page=2", "", "property-repeated"),
        ("box a", "  meta\n", "meta-invalid"),
        ("box a", "  meta \"two words\" { }\n", "meta-invalid"),
        ("box a", "  meta docs other { }\n", "meta-invalid"),
        ("box a", "  meta docs page=1 { }\n", "meta-invalid"),
        ("box a", "  meta docs { page #null }\n", "meta-invalid"),
        ("box a", "  meta docs { a { b c=#null } }\n", "meta-invalid"),
        (
            "box a",
            "  meta docs { }\n  meta other { }\n  meta docs { }\n",
            "meta-twice",
        ),
        // Any other property a node does not know is what it was.
        ("box a colour=\"red\"", "", "property-unknown"),
        ("box a pile=\"@a\"", "", "property-unknown"),
        ("box a id=\"3f9c2b7e0a1d4c5e8f6a7b9c0d1e2f3a\"", "", "property-unknown"),
        ("box a", "  box b\n  line a b colour=\"red\"\n", "property-unknown"),
    ] {
        assert_eq!(codes(&boxed(first, rest)), vec![expected], "{first} {rest}");
    }
    // Where no metadata is taken, a name with a colon is an unknown property.
    for source in [
        "diagram state version=1 {\n  title \"t\"\n  desc \"d\"\n  state a start=#true\n  state b end=#true\n  transition a b on=\"go\" docs:page=\"x\"\n}",
        "diagram sequence version=1 {\n  title \"t\"\n  desc \"d\"\n  actor a\n  actor b\n  call a b \"go\" docs:page=\"x\"\n}",
        "diagram swimlane version=1 {\n  title \"t\"\n  desc \"d\"\n  lane a docs:page=\"x\" {\n    step s\n  }\n}",
        "diagram dependency version=1 docs:page=\"x\" {\n  title \"t\"\n  desc \"d\"\n  box a\n}",
    ] {
        assert_eq!(codes(source), vec!["property-unknown"], "{source}");
    }
    // What a host would refuse reads: nothing inside metadata is checked.
    let diagram = read(&boxed(
        "box a pile:id=\"not an id\" pile:tpye=12 x:y=#true",
        "  meta pile { }\n  meta other {\n    anything goes=\"here\" 1 2.5 #false {\n      deeper \"still\"\n    }\n  }\n",
    ));
    assert_eq!(items(&diagram)[0].meta.len(), 3);
    assert_eq!(diagram.meta.len(), 2);
    assert!(diagram.meta[0].nodes.is_empty());
    let anything = &diagram.meta[1].nodes[0];
    assert_eq!(anything.args.len(), 3);
    assert_eq!(anything.args[1].0, MetaValue::Float(2.5));
    assert_eq!(anything.children[0].name, "deeper");
}
