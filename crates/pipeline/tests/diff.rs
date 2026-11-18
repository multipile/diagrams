//! Two versions of a diagram, drawn as one: still, the second with what was
//! added, removed and changed marked; animated, the first becoming the
//! second. Each case in `cases/diff/<name>/` holds `before.diagram` and
//! `after.diagram`, and the second says in its first line what changed:
//! `// diff: added=<n> removed=<n> changed=<n>`.

mod common;

use common::*;
use diagram_pipeline::{Error, MotionFormat, StillFormat};

struct DiffCase {
    name: String,
    before: String,
    after: String,
}

fn diff_cases() -> Vec<DiffCase> {
    let dir = cases_dir().join("diff");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| DiffCase {
            before: std::fs::read_to_string(dir.join(&name).join("before.diagram")).unwrap(),
            after: std::fs::read_to_string(dir.join(&name).join("after.diagram")).unwrap(),
            name,
        })
        .collect()
}

fn counted(html: &str, what: &str) -> usize {
    let key = format!("data-{what}=\"");
    let start = html.find(&key).unwrap() + key.len();
    html[start..start + html[start..].find('"').unwrap()].parse().unwrap()
}

fn keep(name: &str, extension: &str, bytes: &[u8]) {
    let path = out_dir().join("diff").join(format!("{name}.{extension}"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_diff_counts_what_was_added_removed_and_changed() {
    let cases = diff_cases();
    assert!(cases.len() >= 5);
    for case in cases {
        let first = case.after.lines().next().unwrap();
        let expected = first
            .strip_prefix("// diff: ")
            .expect("the first line says what changed");
        let html = diagram_pipeline::diff_page(&case.before, &case.after, false).unwrap();
        keep(&case.name, "html", html.as_bytes());
        let found = format!(
            "added={} removed={} changed={}",
            counted(&html, "added"),
            counted(&html, "removed"),
            counted(&html, "changed")
        );
        assert_eq!(found, expected, "{}", case.name);
    }
}

#[test]
fn an_unchanged_diagram_marks_nothing() {
    for kind in TYPES {
        let case = &cases(kind)[0];
        let html = diagram_pipeline::diff_page(&case.source, &case.source, false).unwrap();
        for what in ["added", "removed", "changed"] {
            assert_eq!(counted(&html, what), 0, "{}: {what}", case.name);
        }
        assert!(
            !html.contains("class=\"ledger"),
            "{}: a ledger with nothing in it",
            case.name
        );
    }
}

#[test]
fn a_still_diff_marks_each_change_and_lists_it_under_the_diagram() {
    let renderer = renderer();
    let case = diff_cases().into_iter().find(|c| c.name == "a-crate-is-added").unwrap();
    let html = diagram_pipeline::diff_page(&case.before, &case.after, false).unwrap();
    assert!(html.contains("data-id=\"badge:added\""));
    assert!(html.contains("data-id=\"badge:changed\""));
    assert!(
        html.contains("ADDED: pile → diagrams · diagrams → storage · diagrams"),
        "{html}"
    );
    assert!(html.contains("REMOVED: pile → guide · guide"));
    assert!(html.contains("CHANGED: storage"));
    let png = diagram_pipeline::diff_still(&renderer, &case.before, &case.after, StillFormat::Png).unwrap();
    keep(&case.name, "png", &png);
    let picture = Picture::png(&png);
    let green = picture
        .pixels()
        .filter(|p| p[1] > 100 && p[1] as i32 - p[0] as i32 > 40 && p[1] as i32 - p[2] as i32 > 20)
        .count();
    assert!(green > 50, "the added crate is marked green: {green} pixels");
}

#[test]
fn an_element_that_only_moved_is_not_marked() {
    let case = diff_cases().into_iter().find(|c| c.name == "a-box-moves-tier").unwrap();
    let html = diagram_pipeline::diff_page(&case.before, &case.after, false).unwrap();
    // The comment moved to the last tier: it is drawn there, and not marked.
    let comment = html.split("data-id=\"box:comment\"").nth(1).unwrap();
    let comment = &comment[..comment.find("</div>\n").unwrap()];
    assert!(!comment.contains("badge:"), "the moved box is marked: {comment}");
    assert!(html.contains("CHANGED: action → comment"));
}

#[test]
fn a_diff_animation_starts_on_the_first_version_and_ends_on_the_second() {
    let renderer = renderer();
    for case in diff_cases() {
        let gif =
            diagram_pipeline::diff_animation(&renderer, &case.before, &case.after, MotionFormat::Gif, 10).unwrap();
        keep(&case.name, "gif", &gif);
        let frames = gif_frames(&gif);
        assert!(frames.len() > 20, "{}: {} frames", case.name, frames.len());
        let first = Picture::png(
            &diagram_pipeline::diff_at(&renderer, &case.before, &case.after, StillFormat::Png, 0).unwrap(),
        );
        let before = Picture::png(&diagram_pipeline::still(&renderer, &case.before, StillFormat::Png).unwrap());
        // The first moment is the first version, on the larger of the two
        // pictures: each of its pixels is the first version's.
        let differing = (0..before.height.min(first.height))
            .flat_map(|y| (0..before.width.min(first.width)).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let (a, b) = (first.at(x, y), before.at(x, y));
                (0..3).any(|i| (a[i] as i32 - b[i] as i32).abs() > 40)
            })
            .count();
        let share = differing as f32 / (before.width * before.height) as f32;
        assert!(
            share < 0.01,
            "{}: the first moment differs from the first version in {:.1}% of it",
            case.name,
            share * 100.0
        );
        let last = Picture::png(
            &diagram_pipeline::diff_at(&renderer, &case.before, &case.after, StillFormat::Png, 99_999).unwrap(),
        );
        assert_ne!(first.rgba, last.rgba, "{}: the animation changes nothing", case.name);
    }
}

#[test]
fn two_diagrams_of_different_types_are_not_compared() {
    let tree = &cases("tree")[0];
    let state = &cases("state")[0];
    let error = diagram_pipeline::diff_page(&tree.source, &state.source, false).unwrap_err();
    assert!(matches!(error, Error::Diff(_)));
    assert!(
        error
            .to_string()
            .contains("a tree diagram cannot be compared with a state diagram"),
        "{error}"
    );
}

#[test]
fn every_type_can_be_diffed_against_a_changed_copy_of_itself() {
    let renderer = renderer();
    for kind in TYPES {
        let case = &cases(kind)[0];
        // The same diagram with its title changed: nothing in it changed.
        let after = case.source.replacen("title \"", "title \"Now: ", 1);
        let html = diagram_pipeline::diff_page(&case.source, &after, true).unwrap();
        assert_eq!(counted(&html, "changed"), 0, "{}", case.name);
        let gif = diagram_pipeline::diff_animation(&renderer, &case.source, &after, MotionFormat::Gif, 5).unwrap();
        assert!(gif_frames(&gif).len() > 5, "{}", case.name);
    }
}

#[test]
fn a_diagram_marks_the_elements_a_change_touched_by_key() {
    let renderer = renderer();
    let case = cases("dependency")
        .into_iter()
        .find(|c| c.name.ends_with("the-cli-crates"))
        .unwrap();
    let document = diagram_pipeline::changed_document(&case.source, &["storage", "nothing"]).unwrap();
    let html = diagram_html::page(&document, &diagram_pipeline::Options::default());
    assert_eq!(
        counted(&html, "changed"),
        1,
        "only the element the diagram shows is marked"
    );
    assert_eq!(counted(&html, "added") + counted(&html, "removed"), 0);
    assert!(html.contains("CHANGED: storage"), "it is listed under the diagram");
    assert_eq!(document.duration_ms, 0, "it is still");
    let png = renderer.still(&document, StillFormat::Png, None).unwrap();
    keep("the-cli-crates-storage-changed", "png", &png);
    let unmarked = diagram_pipeline::changed_document(&case.source, &[]).unwrap();
    assert!(!diagram_html::page(&unmarked, &diagram_pipeline::Options::default()).contains("data-id=\"badge:"));
    // What the file once called the box's pile is not its key.
    let by_pile = diagram_pipeline::changed_document(&case.source, &["@storage"]).unwrap();
    assert_eq!(by_pile.changes.unwrap().changed, 0);
}

#[test]
fn a_diagram_marks_a_line_by_the_keys_of_its_two_ends() {
    let renderer = renderer();
    let case = cases("dependency")
        .into_iter()
        .find(|c| c.name.ends_with("its-elements-carry-metadata"))
        .unwrap();
    let document =
        diagram_pipeline::changed_document(&case.source, &["command->reader", "reader->command", "model->command"])
            .unwrap();
    assert_eq!(
        document.changes.unwrap().changed,
        2,
        "the line and the cycle, and no line the other way"
    );
    let marked: Vec<&str> = document
        .nodes
        .iter()
        .filter(|node| {
            node.children()
                .iter()
                .any(|child| child.id.as_deref() == Some("badge:changed"))
        })
        .filter_map(|node| node.id.as_deref())
        .filter(|id| !id.starts_with("ledger:"))
        .collect();
    assert_eq!(marked, vec!["line:command->reader", "cycle:model->command"]);
    let html = diagram_html::page(&document, &diagram_pipeline::Options::default());
    assert!(
        html.contains("CHANGED: command → reader · model → command"),
        "they are listed under the diagram"
    );
    let png = renderer.still(&document, StillFormat::Png, None).unwrap();
    keep("a-line-and-a-cycle-changed", "png", &png);
}

#[test]
fn the_command_marks_the_keys_and_the_lines_it_is_given() {
    let case = cases("swimlane")
        .into_iter()
        .find(|c| c.name.ends_with("its-elements-carry-metadata"))
        .unwrap();
    let out = out_dir().join("diff").join("a-refund-marked-by-the-command.html");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_diagram"))
        .arg("render")
        .arg(&case.path)
        .arg("-o")
        .arg(&out)
        .args(["--changed", "pay, open->approve"])
        .status()
        .unwrap();
    assert!(status.success());
    let html = std::fs::read_to_string(&out).unwrap();
    assert_eq!(counted(&html, "changed"), 2);
    assert!(html.contains("CHANGED: open → approve · Pay"), "{}", case.name);
}

#[test]
fn a_nested_level_is_marked_by_its_place_from_the_outside() {
    let case = cases("nested")
        .into_iter()
        .find(|c| c.name.ends_with("which-pile-a-path-is-in"))
        .unwrap();
    let document = diagram_pipeline::changed_document(&case.source, &["level-3"]).unwrap();
    let html = diagram_html::page(&document, &diagram_pipeline::Options::default());
    assert_eq!(counted(&html, "changed"), 1);
    assert!(html.contains("CHANGED: @html"));
}

#[test]
fn metadata_that_changes_alone_is_a_change() {
    let case = cases("swimlane")
        .into_iter()
        .find(|c| c.name.ends_with("its-elements-carry-metadata"))
        .unwrap();
    let diagram = |source: &str| diagram_pipeline::read(source).unwrap().diagram;
    let changed = |after: &str| {
        assert_ne!(case.source, after);
        diagram_document::diff_document(&diagram(&case.source), &diagram(after), false)
            .unwrap()
            .changes
            .unwrap()
    };
    let step = changed(&case.source.replace("sla:hours=4\n", "sla:hours=8\n"));
    assert_eq!((step.added, step.removed, step.changed), (0, 0, 1), "a step's");
    let line = changed(&case.source.replace("sla:queue=\"refunds\"", "sla:queue=\"returns\""));
    assert_eq!((line.added, line.removed, line.changed), (0, 0, 1), "a line's");
    let gained = changed(
        &case
            .source
            .replace("label=\"approved\"", "label=\"approved\" sla:queue=\"payouts\""),
    );
    assert_eq!(gained.changed, 1, "a line that gains some");
    let nested = cases("nested")
        .into_iter()
        .find(|c| c.name.ends_with("its-elements-carry-metadata"))
        .unwrap();
    let after = nested.source.replace("policy:rank=3", "policy:rank=4");
    let level = diagram_document::diff_document(&diagram(&nested.source), &diagram(&after), false)
        .unwrap()
        .changes
        .unwrap();
    assert_eq!(level.changed, 1, "a level's");
}
