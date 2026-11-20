//! Two versions of a diagram, drawn as one: still, the second with what was
//! added, removed and changed marked; animated, the first becoming the
//! second. Each case in `cases/diff/<name>/` holds `before.diagram` and
//! `after.diagram`, and the second says in its first line what changed:
//! `// diff: added=<n> removed=<n> changed=<n>`.

mod common;

use common::*;
use diagram_pipeline::{Error, MotionFormat, Report, StillFormat, Subject};

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
        let html = diagram_pipeline::diff_page(&case.before, &case.after, false, &[]).unwrap();
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
        let html = diagram_pipeline::diff_page(&case.source, &case.source, false, &[]).unwrap();
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
    let html = diagram_pipeline::diff_page(&case.before, &case.after, false, &[]).unwrap();
    assert!(html.contains("data-id=\"badge:added\""));
    assert!(html.contains("data-id=\"badge:changed\""));
    assert!(
        html.contains("ADDED: pile → diagrams · diagrams → storage · diagrams"),
        "{html}"
    );
    assert!(html.contains("REMOVED: pile → guide · guide"));
    assert!(html.contains("CHANGED: storage"));
    let png = diagram_pipeline::diff_still(&renderer, &case.before, &case.after, StillFormat::Png, &[]).unwrap();
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
    let html = diagram_pipeline::diff_page(&case.before, &case.after, false, &[]).unwrap();
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
            diagram_pipeline::diff_animation(&renderer, &case.before, &case.after, MotionFormat::Gif, 10, &[]).unwrap();
        keep(&case.name, "gif", &gif);
        let frames = gif_frames(&gif);
        assert!(frames.len() > 20, "{}: {} frames", case.name, frames.len());
        let first = Picture::png(
            &diagram_pipeline::diff_at(&renderer, &case.before, &case.after, StillFormat::Png, 0, &[]).unwrap(),
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
            &diagram_pipeline::diff_at(&renderer, &case.before, &case.after, StillFormat::Png, 99_999, &[]).unwrap(),
        );
        // A pair that differs in nothing drawn shows nothing happening.
        if case.after.starts_with("// diff: added=0 removed=0 changed=0\n") {
            assert_eq!(first.rgba, last.rgba, "{}: nothing drawn changed", case.name);
        } else {
            assert_ne!(first.rgba, last.rgba, "{}: the animation changes nothing", case.name);
        }
    }
}

#[test]
fn two_diagrams_of_different_types_are_not_compared() {
    let tree = &cases("tree")[0];
    let state = &cases("state")[0];
    let error = diagram_pipeline::diff_page(&tree.source, &state.source, false, &[]).unwrap_err();
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
        let html = diagram_pipeline::diff_page(&case.source, &after, true, &[]).unwrap();
        assert_eq!(counted(&html, "changed"), 0, "{}", case.name);
        let gif = diagram_pipeline::diff_animation(&renderer, &case.source, &after, MotionFormat::Gif, 5, &[]).unwrap();
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
    // Only a key names a box, never what it shows.
    let by_sub = diagram_pipeline::changed_document(&case.source, &["crates/storage"]).unwrap();
    assert_eq!(by_sub.changes.unwrap().changed, 0);
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
fn metadata_that_changes_alone_is_not_a_change() {
    let case = cases("swimlane")
        .into_iter()
        .find(|c| c.name.ends_with("its-elements-carry-metadata"))
        .unwrap();
    let diagram = |source: &str| diagram_pipeline::read(source).unwrap().diagram;
    let compared = |before: &str, after: &str| {
        assert_ne!(before, after);
        let report = diagram_document::compare(&diagram(before), &diagram(after)).unwrap();
        assert!(report.is_empty(), "{report:?}");
        let document = diagram_document::diff_document(&diagram(before), &diagram(after), &report, false).unwrap();
        let html = diagram_html::page(&document, &diagram_pipeline::Options::default());
        assert!(!html.contains("data-id=\"badge:"), "an element is marked");
        assert!(!html.contains("class=\"ledger"), "a ledger with nothing in it");
        let changes = document.changes.unwrap();
        (changes.added, changes.removed, changes.changed)
    };
    let step = case.source.replace("sla:hours=4\n", "sla:hours=8\n");
    assert_eq!(compared(&case.source, &step), (0, 0, 0), "a step's");
    let line = case.source.replace("sla:queue=\"refunds\"", "sla:queue=\"returns\"");
    assert_eq!(compared(&case.source, &line), (0, 0, 0), "a line's");
    let gained = case
        .source
        .replace("label=\"approved\"", "label=\"approved\" sla:queue=\"payouts\"");
    assert_eq!(compared(&case.source, &gained), (0, 0, 0), "a line that gains some");
    assert_eq!(compared(&gained, &case.source), (0, 0, 0), "a line that loses some");
    let nested = cases("nested")
        .into_iter()
        .find(|c| c.name.ends_with("its-elements-carry-metadata"))
        .unwrap();
    let level = nested.source.replace("policy:rank=3", "policy:rank=4");
    assert_eq!(compared(&nested.source, &level), (0, 0, 0), "a level's");
}

#[test]
fn two_versions_that_differ_only_in_metadata_compare_as_unchanged() {
    let dir = cases_dir().join("diff").join("only-the-metadata-changes");
    let before = std::fs::read_to_string(dir.join("before.diagram")).unwrap();
    let after = std::fs::read_to_string(dir.join("after.diagram")).unwrap();
    let html = diagram_pipeline::diff_page(&before, &after, false, &[]).unwrap();
    for what in ["added", "removed", "changed"] {
        assert_eq!(counted(&html, what), 0, "{what}");
    }
    assert!(!html.contains("data-id=\"badge:"), "an element is marked");
    assert!(!html.contains("class=\"ledger"), "a ledger with nothing in it");
    // The second version's metadata is still passed on.
    assert!(html.contains("data-docs-page=\"storage.md\""), "{html}");
}

#[test]
fn a_diff_marks_what_its_caller_adds_to_the_report() {
    let renderer = renderer();
    let dir = cases_dir().join("diff").join("only-the-metadata-changes");
    let before = std::fs::read_to_string(dir.join("before.diagram")).unwrap();
    let after = std::fs::read_to_string(dir.join("after.diagram")).unwrap();
    let diagram = |source: &str| diagram_pipeline::read(source).unwrap().diagram;
    let mut report = diagram_document::compare(&diagram(&before), &diagram(&after)).unwrap();
    assert!(report.is_empty(), "{report:?}");
    report.change(Subject::element("storage"));
    report.change(Subject::line("cli", "piles"));
    let document = diagram_document::diff_document(&diagram(&before), &diagram(&after), &report, false).unwrap();
    let changes = document.changes.unwrap();
    assert_eq!((changes.added, changes.removed, changes.changed), (0, 0, 2));
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
    assert_eq!(marked, vec!["line:cli->piles", "box:storage"]);
    let html = diagram_html::page(&document, &diagram_pipeline::Options::default());
    assert!(html.contains("CHANGED: cli → piles · storage"), "{html}");
    let png = renderer.still(&document, StillFormat::Png, None).unwrap();
    keep("only-the-metadata-changes-storage-marked", "png", &png);

    // Beside what is drawn differently, and never what one version lacks.
    let case = diff_cases().into_iter().find(|c| c.name == "a-crate-is-added").unwrap();
    let plain = diagram_pipeline::compared_document(&case.before, &case.after, false, &[]).unwrap();
    let plain = plain.changes.unwrap();
    let marked =
        diagram_pipeline::compared_document(&case.before, &case.after, false, &["pile", "diagrams", "guide"]).unwrap();
    let marked = marked.changes.unwrap();
    assert_eq!(
        (marked.added, marked.removed, marked.changed),
        (plain.added, plain.removed, plain.changed + 1),
        "the kept box is marked, the added and the removed stay as they are"
    );

    // Animated, an element reported as changed is marked at the end.
    let animated = diagram_pipeline::compared_document(&before, &after, true, &["storage"]).unwrap();
    assert!(animated.duration_ms > 0);
    assert_eq!(animated.changes.unwrap().changed, 1);
}

#[test]
fn the_diff_command_marks_the_keys_and_the_lines_it_is_given() {
    let dir = cases_dir().join("diff").join("only-the-metadata-changes");
    let out = out_dir()
        .join("diff")
        .join("only-the-metadata-changes-marked-by-the-command.html");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let run = |changed: &[&str]| {
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_diagram"))
            .arg("diff")
            .arg(dir.join("before.diagram"))
            .arg(dir.join("after.diagram"))
            .arg("-o")
            .arg(&out)
            .args(changed)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::read_to_string(&out).unwrap()
    };
    assert_eq!(counted(&run(&[]), "changed"), 0);
    let html = run(&["--changed", "storage, cli->piles"]);
    assert_eq!(counted(&html, "changed"), 2);
    assert!(html.contains("CHANGED: cli → piles · storage"), "{html}");
}

#[test]
fn a_comparison_reports_each_difference_by_key_or_by_id() {
    let case = diff_cases().into_iter().find(|c| c.name == "a-crate-is-added").unwrap();
    let report = diagram_pipeline::compare(&case.before, &case.after).unwrap();
    assert_eq!(
        report,
        Report {
            added: vec![
                Subject::element("diagrams"),
                Subject::line("pile", "diagrams"),
                Subject::line("diagrams", "storage"),
            ],
            removed: vec![Subject::element("guide"), Subject::line("pile", "guide")],
            changed: vec![Subject::element("storage")],
        }
    );
    // What a file names by no key is reported by the id its layout gives.
    let sequence =
        "diagram sequence version=1 {\n  title \"t\"\n  desc \"d\"\n  actor a\n  actor b\n  call a b \"ask\"\n}";
    let report = diagram_pipeline::compare(sequence, &sequence.replace("\"ask\"", "\"tell\"")).unwrap();
    assert_eq!(report.added, vec![Subject::shape("message:a->b:tell")]);
    assert_eq!(report.removed, vec![Subject::shape("message:a->b:ask")]);
    // Two lines between the same two boxes are told apart by id.
    let twice = |second: &str| {
        format!(
            "diagram architecture version=1 {{\n  title \"t\"\n  desc \"d\"\n  flow down\n  box a\n  box b\n  tier a\n  tier b\n  line a b label=\"read\"\n{second}}}"
        )
    };
    let report = diagram_pipeline::compare(&twice(""), &twice("  line a b label=\"write\"\n")).unwrap();
    assert_eq!(report.added, vec![Subject::shape("line:a->b#2")]);
    assert!(report.removed.is_empty() && report.changed.is_empty(), "{report:?}");
}

#[test]
fn a_diff_draws_what_its_report_says_and_decides_nothing() {
    let case = diff_cases().into_iter().find(|c| c.name == "a-crate-is-added").unwrap();
    let marked = |report: &Report, animated: bool| {
        let document = diagram_pipeline::diff_document(&case.before, &case.after, report, animated).unwrap();
        let changes = document.changes.unwrap();
        let html = diagram_html::page(&document, &diagram_pipeline::Options::default());
        ((changes.added, changes.removed, changes.changed), html)
    };
    // An empty report: the second version, with nothing marked, though the
    // two versions are drawn differently.
    for animated in [false, true] {
        let (counts, html) = marked(&Report::default(), animated);
        assert_eq!(counts, (0, 0, 0));
        assert!(!html.contains("data-id=\"badge:"), "an element is marked");
        assert!(!html.contains("data-id=\"ledger:"), "a ledger with nothing in it");
        assert!(
            html.contains("data-id=\"box:diagrams\""),
            "the second version's box is drawn"
        );
        assert_eq!(
            html.contains("data-id=\"box:guide\""),
            animated,
            "the first version's box is drawn only to fade"
        );
    }
    // What the report lists is marked as listed, whatever the two versions
    // draw: a box both draw the same as changed, the added box not at all.
    let report = Report {
        changed: vec![Subject::element("pile"), Subject::shape("line:pile->piles")],
        removed: vec![Subject::element("guide")],
        ..Report::default()
    };
    for animated in [false, true] {
        let (counts, html) = marked(&report, animated);
        assert_eq!(counts, (0, 1, 2));
        assert!(html.contains("CHANGED: pile → piles · pile"), "{html}");
        assert!(html.contains("REMOVED: guide"), "{html}");
        assert!(!html.contains("data-id=\"badge:added\""));
    }
    // Passed back as it came, the report draws what a comparison shows.
    let report = diagram_pipeline::compare(&case.before, &case.after).unwrap();
    assert_eq!(marked(&report, false).0, (3, 2, 1));
}

#[test]
fn a_still_diff_leaves_out_a_removed_line_that_would_muddle_the_second_version() {
    // guide is removed where diagrams is added, so neither guide nor the
    // line to it is drawn: the line would seem to lead to diagrams. Both
    // are still listed under the diagram.
    let case = diff_cases().into_iter().find(|c| c.name == "a-crate-is-added").unwrap();
    let report = diagram_pipeline::compare(&case.before, &case.after).unwrap();
    let document = diagram_pipeline::diff_document(&case.before, &case.after, &report, false).unwrap();
    let ids: Vec<&str> = document.nodes.iter().filter_map(|node| node.id.as_deref()).collect();
    assert!(!ids.contains(&"box:guide"), "{ids:?}");
    assert!(!ids.contains(&"line:pile->guide"), "{ids:?}");
    let html = diagram_html::page(&document, &diagram_pipeline::Options::default());
    assert!(html.contains("REMOVED: pile → guide · guide"));
    // Animated, it still fades out.
    let animated = diagram_pipeline::diff_document(&case.before, &case.after, &report, true).unwrap();
    assert!(
        animated
            .nodes
            .iter()
            .any(|node| node.id.as_deref() == Some("line:pile->guide"))
    );
}

#[test]
fn a_still_diff_puts_each_lines_badge_clear_of_every_other_line() {
    use diagram_document::document::Place;
    for case in diff_cases() {
        let report = diagram_pipeline::compare(&case.before, &case.after).unwrap();
        let document = diagram_pipeline::diff_document(&case.before, &case.after, &report, false).unwrap();
        let scene = diagram_document::layout(&diagram_pipeline::read(&case.after).unwrap().diagram);
        for node in &document.nodes {
            let (Some(id), Place::Absolute(at)) = (node.id.as_deref(), &node.place) else {
                continue;
            };
            if !id.starts_with("line:") {
                continue;
            }
            for badge in node
                .children()
                .iter()
                .filter(|child| child.id.as_deref().is_some_and(|id| id.starts_with("badge:")))
            {
                let Place::Absolute(rect) = badge.place else { continue };
                let rect = rect.translate(at.x, at.y);
                for other in &scene.elements {
                    let diagram_document::scene::Shape::Line(line) = &other.shape else {
                        continue;
                    };
                    if other.id == id {
                        continue;
                    }
                    assert!(
                        line.points.windows(2).all(|s| !rect.crossed_by(s[0], s[1])),
                        "{}: the badge of {id} sits on {}",
                        case.name,
                        other.id
                    );
                }
            }
        }
    }
}
