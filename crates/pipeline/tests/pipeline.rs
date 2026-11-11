//! Every case, from its file to a picture: read, built into a page, and
//! rendered still and revealed. What each step makes is kept in
//! `target/tmp/e2e/`, to look at.

mod common;

use common::*;
use diagram_pipeline::{Animation, MotionFormat, StillFormat, format::Format};

#[test]
fn every_type_has_several_cases() {
    for kind in TYPES {
        let count = cases(kind).len();
        assert!(
            count >= 3,
            "cases/{kind} has {count} cases, and every type has at least 3"
        );
    }
}

#[test]
fn every_case_reads_as_the_type_its_directory_names() {
    for case in all_cases() {
        let read = diagram_pipeline::read(&case.source).unwrap_or_else(|error| panic!("{}:\n{error}", case.name));
        let kind = case
            .path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(read.diagram.kind().word(), kind, "{}", case.name);
        assert_eq!(read.format, Format::Kdl { version: 1 }, "{}", case.name);
    }
}

#[test]
fn every_case_builds_a_page_that_loads_and_runs_nothing() {
    for case in all_cases() {
        let html = diagram_pipeline::page(&case.source, Animation::None).unwrap();
        case.keep("html", html.as_bytes());
        let lower = html.to_lowercase();
        for forbidden in [
            "<script",
            "<link",
            "<iframe",
            "<img",
            "src=",
            "url(http",
            "@import",
            "javascript:",
        ] {
            assert!(!lower.contains(forbidden), "{}: the page holds {forbidden}", case.name);
        }
        assert!(html.starts_with("<!DOCTYPE html>"), "{}", case.name);
        assert!(
            lower.contains("@font-face"),
            "{}: the page carries its fonts",
            case.name
        );
        let diagram = case.diagram();
        assert!(
            html.contains(&diagram_html::escape(&diagram.frame.title)),
            "{}: the title is drawn",
            case.name
        );
        assert!(
            html.contains("role=\"img\""),
            "{}: the diagram is an image to a screen reader",
            case.name
        );
    }
}

#[test]
fn every_element_of_a_case_is_on_its_page() {
    for case in all_cases() {
        let diagram = case.diagram();
        let scene = diagram_document::layout(&diagram);
        let html = diagram_pipeline::page(&case.source, Animation::None).unwrap();
        for element in &scene.elements {
            let id = format!("data-id=\"{}\"", diagram_html::escape(&element.id));
            assert!(html.contains(&id), "{}: {} is not on the page", case.name, element.id);
        }
        // Every element is inside the picture.
        for element in &scene.elements {
            let bounds = element.shape.bounds();
            assert!(
                bounds.x >= 0 && bounds.y >= 0 && bounds.right() <= scene.width && bounds.bottom() <= scene.height,
                "{}: {} is drawn at {bounds:?}, outside the {}×{} picture",
                case.name,
                element.id,
                scene.width,
                scene.height
            );
        }
    }
}

#[test]
fn every_case_renders_a_still_picture() {
    let renderer = renderer();
    for case in all_cases() {
        let png = diagram_pipeline::still(&renderer, &case.source, StillFormat::Png).unwrap();
        case.keep("png", &png);
        let scene = diagram_document::layout(&case.diagram());
        let picture = Picture::png(&png);
        assert_eq!(
            (picture.width as i32, picture.height as i32),
            (scene.width, scene.height),
            "{}",
            case.name
        );
        let share = picture.inked() as f32 / (picture.width * picture.height) as f32;
        assert!(
            share > 0.02,
            "{}: only {:.1}% of the picture is drawn",
            case.name,
            share * 100.0
        );
        // The paper is in the corner, where nothing is drawn.
        assert_eq!(picture.at(2, 2), [0xf5, 0xf5, 0xf5, 255], "{}", case.name);
    }
}

#[test]
fn a_focal_element_is_drawn_in_the_accent() {
    let renderer = renderer();
    for case in all_cases() {
        let focal = case.source.contains("focal=#true");
        let png = diagram_pipeline::still(&renderer, &case.source, StillFormat::Png).unwrap();
        let accented = Picture::png(&png).accented();
        if focal {
            assert!(
                accented > 50,
                "{}: a focal element, and only {accented} pixels in the accent",
                case.name
            );
        }
    }
}

#[test]
fn every_case_is_revealed_step_by_step() {
    let renderer = renderer();
    for case in all_cases() {
        let gif = diagram_pipeline::reveal(&renderer, &case.source, MotionFormat::Gif, 10).unwrap();
        case.keep("gif", &gif);
        let frames = gif_frames(&gif);
        assert!(
            frames.len() > 10,
            "{}: the reveal has {} frames",
            case.name,
            frames.len()
        );
        let (first, last) = (&frames[0].0, &frames[frames.len() - 1].0);
        assert_ne!(first, last, "{}: the reveal does not change the picture", case.name);
        // The first moment shows less than the last.
        let start = Picture::png(&diagram_pipeline::reveal_at(&renderer, &case.source, StillFormat::Png, 0).unwrap());
        let end = Picture::png(&diagram_pipeline::still(&renderer, &case.source, StillFormat::Png).unwrap());
        assert!(
            start.inked() < end.inked(),
            "{}: the reveal starts with everything drawn",
            case.name
        );
    }
}

#[test]
fn a_reveal_ends_on_the_still_picture() {
    let renderer = renderer();
    for kind in TYPES {
        let case = &cases(kind)[0];
        let revealed = diagram_pipeline::document(&case.source, Animation::Reveal).unwrap();
        let end = renderer.still(&revealed, StillFormat::Png, None).unwrap();
        let still = diagram_pipeline::still(&renderer, &case.source, StillFormat::Png).unwrap();
        assert_eq!(Picture::png(&end).rgba, Picture::png(&still).rgba, "{}", case.name);
    }
}

#[test]
fn a_picture_comes_out_the_same_every_time() {
    let renderer = renderer();
    for kind in TYPES {
        let case = &cases(kind)[0];
        let first = diagram_pipeline::still(&renderer, &case.source, StillFormat::Png).unwrap();
        let second = diagram_pipeline::still(&renderer, &case.source, StillFormat::Png).unwrap();
        assert_eq!(first, second, "{}: two renders differ", case.name);
        let page = diagram_pipeline::page(&case.source, Animation::Reveal).unwrap();
        assert_eq!(
            page,
            diagram_pipeline::page(&case.source, Animation::Reveal).unwrap(),
            "{}",
            case.name
        );
    }
}

#[test]
fn every_type_renders_to_every_format() {
    let renderer = renderer();
    for kind in TYPES {
        let case = &cases(kind)[0];
        let webp = diagram_pipeline::still(&renderer, &case.source, StillFormat::WebP).unwrap();
        assert_eq!(&webp[..4], b"RIFF", "{}", case.name);
        assert_eq!(&webp[8..12], b"WEBP", "{}", case.name);
        let apng = diagram_pipeline::reveal(&renderer, &case.source, MotionFormat::Apng, 5).unwrap();
        assert_eq!(&apng[..8], b"\x89PNG\r\n\x1a\n", "{}", case.name);
        assert!(
            apng.windows(4).any(|w| w == b"acTL"),
            "{}: an APNG has an animation control chunk",
            case.name
        );
        let animated = diagram_pipeline::reveal(&renderer, &case.source, MotionFormat::WebP, 5).unwrap();
        assert!(
            animated.windows(4).any(|w| w == b"ANIM"),
            "{}: an animated WebP has an ANIM chunk",
            case.name
        );
    }
}

#[test]
fn a_picture_at_twice_the_scale_is_twice_the_size() {
    let case = &cases("dependency")[0];
    let one = Picture::png(&diagram_pipeline::still(&renderer(), &case.source, StillFormat::Png).unwrap());
    let two = Picture::png(
        &diagram_pipeline::still(&diagram_pipeline::Renderer::new(), &case.source, StillFormat::Png).unwrap(),
    );
    assert_eq!((two.width, two.height), (one.width * 2, one.height * 2));
}

#[test]
fn the_page_and_the_picture_are_written_from_one_document() {
    for case in all_cases() {
        for animation in [Animation::None, Animation::Reveal] {
            let document = diagram_pipeline::document(&case.source, animation).unwrap();
            let html = diagram_html::page(&document, &diagram_pipeline::Options::default());
            // A <div> for the diagram, and one for each node of its document.
            let nodes = document.walk().len();
            assert_eq!(html.matches("<div").count(), nodes + 1, "{}", case.name);
            for node in document.walk() {
                if let Some(id) = &node.id {
                    let attribute = format!("data-id=\"{}\"", diagram_html::escape(id));
                    assert!(html.contains(&attribute), "{}: {id} is not on the page", case.name);
                }
            }
            // Every track moves the page as it moves the picture.
            for index in 0..document.tracks.len() {
                assert!(
                    html.contains(&format!("@keyframes t{index} ")),
                    "{}: track {index}",
                    case.name
                );
            }
            assert_eq!(
                animation == Animation::None,
                document.tracks.is_empty(),
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn a_line_is_drawn_with_boxes_and_no_svg() {
    for case in all_cases() {
        let html = diagram_pipeline::page(&case.source, Animation::None).unwrap();
        assert!(!html.contains("<svg"), "{}: the page holds SVG", case.name);
    }
}

#[test]
fn a_fragment_embeds_in_another_page() {
    let case = &cases("dependency")[0];
    let document = diagram_pipeline::document(&case.source, Animation::Reveal).unwrap();
    let fragment = diagram_html::fragment(&document, &diagram_pipeline::Options { embed_fonts: false });
    assert!(fragment.starts_with("<div class=\"diagram\""));
    assert!(!fragment.contains("<html") && !fragment.contains("<body"));
    assert!(
        !fragment.contains("@font-face"),
        "a fragment embedded where the fonts are carries none"
    );
    assert!(fragment.contains("@keyframes t0 "), "it carries its own animation");
}

#[test]
fn every_style_on_a_page_is_whole() {
    // A style that held a double quote would end its attribute early, and
    // a browser would drop the rest of it: every style closes its tag.
    for case in all_cases() {
        let html = diagram_pipeline::page(&case.source, Animation::Reveal).unwrap();
        for (n, rest) in html.split(" style=\"").skip(1).enumerate() {
            let value = &rest[..rest.find('"').unwrap()];
            let after = &rest[value.len() + 1..];
            assert!(after.starts_with('>'), "{}: style {n} ends early: {value}", case.name);
            assert!(value.ends_with(';'), "{}: style {n} is cut: {value}", case.name);
        }
    }
}
