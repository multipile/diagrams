//! Writes a diagram's document as one self-contained HTML page, to embed or
//! to open as it is.
//!
//! Every node of the document becomes a `<div>` with its style inline, and
//! every track a CSS `@keyframes`, timed by the same curves the renderer
//! uses. The page loads nothing and runs nothing: its fonts, when it carries
//! them, are `data:` URLs. Since the renderer draws the same document, a
//! page in a browser and a picture show the same thing.
//!
//! The diagram is the page's one `.diagram` element. Its `data-width`,
//! `data-height` and `data-duration-ms` say how large it is and how long it
//! moves, and a diff's `data-added`, `data-removed` and `data-changed` what
//! changed.
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
//! let document = diagram_document::document(&read.diagram, diagram_document::Animation::None);
//! let html = diagram_html::page(&document, &diagram_html::Options::default());
//! assert!(html.contains("data-id=\"box:storage\""));
//! ```

use std::fmt::Write as _;

use base64::Engine as _;
use diagram_document::document::*;
use diagram_document::fonts;
use diagram_document::theme::PAPER;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Whether the page carries its fonts, so it looks the same in any
    /// browser. A page embedded where the fonts already are needs no copy.
    pub embed_fonts: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options { embed_fonts: true }
    }
}

/// The whole page: a document with its `<head>`, to open as it is.
pub fn page(document: &Document, options: &Options) -> String {
    let mut html = String::new();
    let _ = writeln!(html, "<!DOCTYPE html>");
    let _ = writeln!(html, "<html lang=\"en\">");
    let _ = writeln!(html, "<head>");
    let _ = writeln!(html, "<meta charset=\"utf-8\">");
    let _ = writeln!(html, "<title>{}</title>", escape(&document.title));
    let _ = writeln!(
        html,
        "<meta name=\"description\" content=\"{}\">",
        escape(&document.desc)
    );
    let _ = writeln!(
        html,
        "<style>\nhtml, body {{ margin: 0; padding: 0; background: {}; }}\n</style>",
        color(PAPER)
    );
    let _ = writeln!(html, "</head>");
    let _ = writeln!(html, "<body>");
    html.push_str(&fragment(document, options));
    let _ = writeln!(html, "</body>");
    let _ = writeln!(html, "</html>");
    html
}

/// The diagram alone, as one element with its own `<style>`, to embed in a
/// page of something else.
pub fn fragment(document: &Document, options: &Options) -> String {
    let mut css = String::new();
    if options.embed_fonts {
        css.push_str(&font_faces());
    }
    for (index, track) in document.tracks.iter().enumerate() {
        keyframes(&mut css, index, track, document.duration_ms);
    }
    let mut extra = String::new();
    if let Some(changes) = document.changes {
        let _ = write!(
            extra,
            " data-added=\"{}\" data-removed=\"{}\" data-changed=\"{}\"",
            changes.added, changes.removed, changes.changed
        );
    }
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<div class=\"diagram\" role=\"img\" aria-label=\"{}\" data-width=\"{}\" data-height=\"{}\" data-duration-ms=\"{}\"{extra} style=\"position: relative; overflow: hidden; width: {}px; height: {}px; background: {}; font-family: {SANS};\">",
        escape(&format!("{}. {}", document.title, document.desc)),
        document.width,
        document.height,
        document.duration_ms,
        document.width,
        document.height,
        color(PAPER)
    );
    if !css.is_empty() {
        let _ = writeln!(out, "<style>\n{css}</style>");
    }
    for node in &document.nodes {
        write_node(&mut out, node, document.duration_ms);
        out.push('\n');
    }
    let _ = writeln!(out, "</div>");
    out
}

const SANS: &str = "'Geist', system-ui, sans-serif";
const MONO: &str = "'Geist Mono', ui-monospace, monospace";

fn write_node(out: &mut String, node: &Node, duration_ms: u32) {
    let mut style = String::new();
    match node.place {
        Place::Absolute(r) => {
            let _ = write!(
                style,
                "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; ",
                r.x, r.y, r.w, r.h
            );
        }
        Place::Sized { w, h } => {
            let _ = write!(
                style,
                "position: relative; flex-shrink: 0; width: {w}px; height: {h}px; "
            );
        }
        Place::Flow => style.push_str("position: relative; flex-shrink: 0; "),
    }
    style.push_str("box-sizing: border-box; ");
    let s = &node.style;
    if let Some(flex) = s.flex {
        let _ = write!(
            style,
            "display: flex; flex-direction: {}; align-items: {}; justify-content: {}; ",
            if flex.column { "column" } else { "row" },
            align(flex.align),
            align(flex.justify)
        );
    }
    if let Some(background) = s.background {
        let _ = write!(style, "background-color: {}; ", color(background));
    }
    for (side, edge) in ["top", "right", "bottom", "left"].iter().zip(&s.border) {
        if let Some(edge) = edge {
            let kind = match edge.style {
                LineStyle::Solid => "solid",
                LineStyle::Dashed => "dashed",
            };
            let _ = write!(
                style,
                "border-{side}: {}px {kind} {}; ",
                number(edge.width),
                color(edge.color)
            );
        }
    }
    if s.radius.iter().any(|r| *r > 0.0) {
        let [a, b, c, d] = s.radius;
        let _ = write!(
            style,
            "border-radius: {}px {}px {}px {}px; ",
            number(a),
            number(b),
            number(c),
            number(d)
        );
    }
    if s.padding.iter().any(|p| *p != 0) {
        let [a, b, c, d] = s.padding;
        let _ = write!(style, "padding: {a}px {b}px {c}px {d}px; ");
    }
    if let Some(opacity) = s.opacity {
        let _ = write!(style, "opacity: {}; ", number(opacity));
    }
    if let Some(degrees) = s.rotate {
        let _ = write!(style, "transform: rotate({}deg); ", number(degrees));
    }
    if s.clip {
        style.push_str("overflow: hidden; ");
    }
    if let Some(track) = node.track {
        let _ = write!(style, "animation: t{track} {duration_ms}ms linear both; ");
    }
    if let Content::Text(text) = &node.content {
        let _ = write!(
            style,
            "font-family: {}; font-size: {}px; font-weight: {}; color: {}; line-height: {}px; white-space: nowrap; ",
            match text.family {
                Family::Sans => SANS,
                Family::Mono => MONO,
            },
            number(text.size),
            text.weight,
            color(text.color),
            number(text.line_height)
        );
        if text.letter_spacing != 0.0 {
            let _ = write!(style, "letter-spacing: {}px; ", number(text.letter_spacing));
        }
        if text.align == TextAlign::Center {
            style.push_str("text-align: center; ");
        }
        if text.strike {
            style.push_str("text-decoration: line-through; ");
        }
    }
    out.push_str("<div");
    if let Some(id) = &node.id {
        let _ = write!(out, " data-id=\"{}\"", escape(id));
    }
    if let Some(pile) = &node.pile {
        let _ = write!(out, " data-pile=\"{}\"", escape(pile));
    }
    // The style is an attribute's value: whatever it holds is escaped.
    let _ = write!(out, " style=\"{}\">", escape(style.trim_end()));
    match &node.content {
        Content::None => {}
        Content::Text(text) => out.push_str(&escape(&text.value)),
        Content::Children(children) => {
            for child in children {
                write_node(out, child, duration_ms);
            }
        }
    }
    out.push_str("</div>");
}

/// A track as CSS keyframes over the document's whole animation. Each
/// stretch between two keys runs on its key's curve.
fn keyframes(css: &mut String, index: usize, track: &Track, duration_ms: u32) {
    let total = duration_ms.max(1) as f32;
    let pose = |pose: Pose| {
        format!(
            "opacity: {}; translate: {}px {}px;",
            number(pose.opacity),
            number(pose.dx),
            number(pose.dy)
        )
    };
    let _ = writeln!(css, "@keyframes t{index} {{");
    let first = track.keys.first().map_or(Pose::REST, |key| key.pose);
    let _ = writeln!(css, "  0% {{ {} }}", pose(first));
    for (n, key) in track.keys.iter().enumerate() {
        // CSS times the stretch after a keyframe by that keyframe's curve.
        let next = track.keys.get(n + 1).map_or(Ease::Linear, |key| key.ease);
        let (x1, y1, x2, y2) = next.bezier();
        let _ = writeln!(
            css,
            "  {}% {{ {} animation-timing-function: cubic-bezier({}, {}, {}, {}); }}",
            number(key.at_ms as f32 / total * 100.0),
            pose(key.pose),
            number(x1),
            number(y1),
            number(x2),
            number(y2)
        );
    }
    let last = track.keys.last().map_or(Pose::REST, |key| key.pose);
    let _ = writeln!(css, "  100% {{ {} }}", pose(last));
    let _ = writeln!(css, "}}");
}

fn align(align: Align) -> &'static str {
    match align {
        Align::Start => "flex-start",
        Align::Center => "center",
        Align::End => "flex-end",
    }
}

fn color(c: Rgba) -> String {
    if c.3 == 255 {
        format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
    } else {
        format!("rgba({}, {}, {}, {})", c.0, c.1, c.2, number(c.3 as f32 / 255.0))
    }
}

/// A number as CSS takes it: at most three decimals, and none when whole.
fn number(value: f32) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

/// `@font-face` rules that embed every font as a `data:` URL.
pub fn font_faces() -> String {
    fonts::FONTS
        .iter()
        .map(|font| {
            format!(
                "@font-face {{ font-family: \"{}\"; font-weight: 100 900; src: url(data:font/woff2;base64,{}) format(\"woff2\"); }}\n",
                font.family,
                base64::engine::general_purpose::STANDARD.encode(font.woff2)
            )
        })
        .collect()
}

/// Text made safe to put in HTML, as text or in a quoted attribute.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_written_as_css_takes_it() {
        assert_eq!(number(1.0), "1");
        assert_eq!(number(1.5), "1.5");
        assert_eq!(number(0.33333), "0.333");
        assert_eq!(number(-0.17), "-0.17");
    }

    #[test]
    fn a_colour_is_hex_when_opaque() {
        assert_eq!(color(Rgba::hex(0xbf4520)), "#bf4520");
        assert_eq!(color(Rgba(191, 69, 32, 0)), "rgba(191, 69, 32, 0)");
    }
}
