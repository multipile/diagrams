//! A scene painted into a document: every element of the layout turned into
//! the boxes and text that draw it, in the theme's look, and every
//! animation into tracks.

use diagram_format::Look;

use crate::Animation;
use crate::diff::{Diff, Role};
use crate::document::*;
use crate::geometry::{self, MARGIN};
use crate::layout::NOTE_H;
use crate::scene::{
    self, BoxShape, Element, Head, LineShape, Point, Rect, RegionKind, RegionShape, Scene, Shape, Status, Tone,
};
use crate::theme::{self, *};

/// A scene's document, still or revealed.
pub fn document(scene: &Scene, animation: Animation) -> Document {
    let mut painter = Painter::default();
    let duration_ms = match animation {
        Animation::None => 0,
        Animation::Reveal => scene.last_step() * STEP_MS + FADE_MS + HOLD_MS,
    };
    let mut nodes = vec![heading(scene)];
    for element in &scene.elements {
        let track = match animation {
            Animation::None => None,
            Animation::Reveal => Some(painter.reveal(element.step)),
        };
        nodes.push(painter.element(element, &Marking::None).with_track(track));
    }
    nodes.extend(note(scene, 0));
    Document {
        width: scene.width,
        height: scene.height,
        title: scene.title.clone(),
        desc: scene.desc.clone(),
        duration_ms,
        changes: None,
        nodes,
        tracks: painter.tracks,
    }
}

/// The document of two versions of a diagram. Still, it is the second
/// version with what was added, removed and changed marked, and listed
/// under it. Animated, it moves from the first version to the second: what
/// was removed fades, what moved slides to where it is now, what changed
/// fades from its first version to its second, what was added appears, and
/// then the marks.
pub fn diff_document(diff: &Diff, animated: bool) -> Document {
    use theme::diff_timing::*;
    let mut painter = Painter::default();
    let mark = animated.then(|| painter.track(Track::change(HIDDEN, Pose::REST, MARK_AT, MARK_MS, Ease::Out)));
    let mut nodes = vec![heading(&diff.scene)];
    for item in &diff.elements {
        if !animated && item.role == Role::Old {
            continue;
        }
        if !animated && item.role == Role::Removed && overlaps_second(diff, &item.element) {
            continue;
        }
        let track = if animated {
            match item.role {
                Role::Kept => None,
                Role::Moved { dx, dy } => Some(painter.track(Track::change(
                    Pose {
                        dx: dx as f32,
                        dy: dy as f32,
                        ..Pose::REST
                    },
                    Pose::REST,
                    MOVE_AT,
                    MOVE_MS,
                    Ease::InOut,
                ))),
                Role::Added => Some(painter.track(Track::change(HIDDEN, Pose::REST, ADD_AT, ADD_MS, Ease::Out))),
                Role::Removed => Some(painter.track(Track::change(Pose::REST, HIDDEN, REMOVE_AT, REMOVE_MS, Ease::In))),
                Role::Old => Some(painter.track(Track::change(Pose::REST, HIDDEN, MOVE_AT, MOVE_MS, Ease::InOut))),
                Role::New => Some(painter.track(Track::change(HIDDEN, Pose::REST, MOVE_AT, MOVE_MS, Ease::InOut))),
            }
        } else {
            None
        };
        let marking = if animated {
            Marking::Animated { track: mark }
        } else if item.role == Role::Removed {
            Marking::Ghost
        } else {
            Marking::Still
        };
        nodes.push(painter.element(&item.element, &marking).with_track(track));
    }

    let ledger = ledger(diff);
    let ledger_h = ledger.len() as i32 * 18;
    let height = diff.scene.height + ledger_h;
    let top = diff.scene.height - MARGIN + 8;
    for (n, (status, text)) in ledger.into_iter().enumerate() {
        let row = Node::absolute(
            Rect::new(MARGIN, top + n as i32 * 18, diff.scene.width - 2 * MARGIN, 18),
            Style::default().flex(false, Align::Center, Align::Start),
        )
        .with_children(vec![
            badge(status, 14, Place::Sized { w: 14, h: 14 }, None),
            Node {
                style: Style {
                    padding: [0, 0, 0, 8],
                    ..Style::default()
                },
                ..Node::text(mono(&text, 9.0, MUTED))
            },
        ])
        .with_id(format!("ledger:{n}"))
        .with_track(mark);
        nodes.push(row);
    }
    let mut scene = diff.scene.clone();
    scene.height = height;
    nodes.extend(note(&scene, ledger_h));
    Document {
        width: scene.width,
        height,
        title: scene.title.clone(),
        desc: scene.desc.clone(),
        duration_ms: if animated { TOTAL_MS } else { 0 },
        changes: Some(Changes {
            added: diff.count(Status::Added),
            removed: diff.count(Status::Removed),
            changed: diff.count(Status::Changed),
        }),
        nodes,
        tracks: painter.tracks,
    }
}

const HIDDEN: Pose = Pose {
    opacity: 0.0,
    dx: 0.0,
    dy: 0.0,
};

/// How an element shows how it changed.
enum Marking {
    /// It is not part of a diff.
    None,
    /// A still diff: an outline and a badge for a change.
    Still,
    /// A still diff's removed element, drawn faded.
    Ghost,
    /// An animated diff: a badge that appears on the track given.
    Animated { track: Option<usize> },
}

#[derive(Default)]
struct Painter {
    tracks: Vec<Track>,
    reveals: Vec<(u32, usize)>,
}

impl Painter {
    fn track(&mut self, track: Track) -> usize {
        self.tracks.push(track);
        self.tracks.len() - 1
    }

    /// The track of a reveal's step: hidden until its turn, then fading in.
    fn reveal(&mut self, step: u32) -> usize {
        if let Some((_, track)) = self.reveals.iter().find(|(s, _)| *s == step) {
            return *track;
        }
        let track = self.track(Track::change(HIDDEN, Pose::REST, step * STEP_MS, FADE_MS, Ease::Out));
        self.reveals.push((step, track));
        track
    }

    fn element(&mut self, element: &Element, marking: &Marking) -> Node {
        let status = element.status;
        let mut node = match &element.shape {
            Shape::Box(shape) => box_node(shape, status, marking),
            Shape::Region(shape) => region_node(shape),
            Shape::Text(shape) => {
                let color = if shape.focal { ACCENT } else { MUTED };
                Node::absolute(
                    Rect::new(
                        shape.at.x,
                        shape.at.y,
                        shape.width.max(geometry::mono_width(&shape.text)),
                        14,
                    ),
                    Style::default().flex(false, Align::Center, Align::Start),
                )
                .with_children(vec![Node::text(mono(&shape.text, 9.0, color))])
            }
            Shape::Dot(shape) => dot(shape),
            Shape::Line(shape) => line_node(shape, status, marking),
        };
        node.id = Some(element.id.clone());
        if let Shape::Box(shape) = &element.shape {
            node.pile = shape.pile.clone();
        }
        if matches!(marking, Marking::Ghost) {
            node.style.opacity = Some(0.4);
        }
        // A badge on what changed, at the top left of a box or a region.
        if status != Status::Same && !matches!(element.shape, Shape::Line(_)) {
            let track = match marking {
                Marking::Animated { track } => *track,
                _ => None,
            };
            if !matches!(marking, Marking::None) {
                let mut children = match std::mem::replace(&mut node.content, Content::None) {
                    Content::Children(children) => children,
                    _ => Vec::new(),
                };
                children.push(badge(status, 18, Place::Absolute(Rect::new(-9, -9, 18, 18)), track));
                node.content = Content::Children(children);
            }
        }
        node
    }
}

fn heading(scene: &Scene) -> Node {
    Node::absolute(
        Rect::new(MARGIN, MARGIN, scene.width - 2 * MARGIN, 24),
        Style::default().flex(false, Align::Center, Align::Start),
    )
    .with_children(vec![Node::text(
        Text::new(scene.title.clone(), Family::Sans, 17.0, INK)
            .weight(600)
            .spacing(-0.17)
            .line_height(24.0),
    )])
    .with_id("heading")
}

/// The note, above `below` pixels the document keeps under it.
fn note(scene: &Scene, below: i32) -> Option<Node> {
    let note = scene.note.as_ref()?;
    Some(
        Node::absolute(
            Rect::new(
                MARGIN,
                scene.height - below - MARGIN - NOTE_H + 12,
                scene.width - 2 * MARGIN,
                16,
            ),
            Style::default().flex(false, Align::Center, Align::Start),
        )
        .with_children(vec![Node::text(
            Text::new(note.clone(), Family::Sans, 11.0, MUTED).line_height(16.0),
        )])
        .with_id("note"),
    )
}

/// Monospace text: second lines, labels, tags.
fn mono(text: &str, size: f32, color: Rgba) -> Text {
    Text::new(text, Family::Mono, size, color)
        .spacing((size * 0.04 * 100.0).round() / 100.0)
        .line_height(14.0)
}

fn badge(status: Status, size: i32, place: Place, track: Option<usize>) -> Node {
    let (glyph, color) = match status {
        Status::Added => ("+", ADDED),
        Status::Removed => ("−", REMOVED),
        _ => ("~", CHANGED),
    };
    let font = if size >= 18 { 12.0 } else { 10.0 };
    Node::new(
        place,
        Style::default()
            .background(color)
            .radius_all(size as f32 / 2.0)
            .flex(false, Align::Center, Align::Center),
    )
    .with_children(vec![Node::text(
        Text::new(glyph, Family::Mono, font, WHITE)
            .weight(600)
            .line_height(size as f32)
            .centered(),
    )])
    .with_id(format!(
        "badge:{}",
        match status {
            Status::Added => "added",
            Status::Removed => "removed",
            _ => "changed",
        }
    ))
    .with_track(track)
}

fn box_node(shape: &BoxShape, status: Status, marking: &Marking) -> Node {
    let r = shape.rect;
    let mut style = Style::default()
        .background(WHITE)
        .border_all(Edge::solid(1.0, INK))
        .radius_all(6.0)
        .padding(4, 8)
        .flex(true, Align::Center, Align::Center);
    let (mut name_color, mut sub_color) = (INK, MUTED);
    match shape.look {
        Look::Plain => {}
        Look::Store => style = style.background(STORE).radius_all(10.0),
        Look::External => {
            style = style.background(PAPER).border_all(Edge::dashed(1.0, MUTED));
            name_color = MUTED;
        }
        Look::Input => {
            style = style.background(INK);
            name_color = WHITE;
            sub_color = RULE;
        }
        Look::Optional => style = style.border_all(Edge::dashed(1.0, INK)),
        Look::Security => style = style.border_all(Edge::solid(2.0, INK)),
    }
    if shape.focal {
        style = style.background(ACCENT_TINT).border_all(Edge::solid(1.5, ACCENT));
        name_color = ACCENT;
        sub_color = ACCENT;
    }
    if let Some(strength) = shape.fill {
        let base = if shape.focal { ACCENT } else { INK };
        style = style
            .background(base.alpha(strength))
            .border_all(Edge::solid(1.0, base))
            .radius_all(2.0);
        name_color = if strength > 0.45 { WHITE } else { INK };
        sub_color = name_color.alpha(0.8);
    }
    if shape.top {
        style = style.padding(10, 12).flex(true, Align::Start, Align::Start);
    }
    let align = |text: Text| if shape.top { text } else { text.centered() };
    let mut children = vec![];
    if !shape.name.is_empty() {
        let mut name = Text::new(shape.name.clone(), Family::Sans, 12.0, name_color)
            .weight(600)
            .line_height(16.0);
        name.strike = matches!(marking, Marking::Ghost);
        children.push(Node::text(align(name)));
    }
    for line in [&shape.sub, &shape.foot].into_iter().flatten() {
        children.push(Node::text(align(
            Text::new(line.clone(), Family::Mono, 10.0, sub_color).line_height(14.0),
        )));
    }
    if let Some(corner) = &shape.corner {
        let w = geometry::snap((corner.chars().count() as f32 * 5.8) as i32 + 10);
        children.push(
            Node::absolute(
                Rect::new(r.w - 8 - w, -8, w, 16),
                Style::default()
                    .background(PAPER)
                    .border_all(Edge::solid(1.0, RULE))
                    .radius_all(3.0)
                    .flex(false, Align::Center, Align::Center),
            )
            .with_children(vec![Node::text(mono(corner, 9.0, MUTED).centered())]),
        );
    }
    if !shape.tags.is_empty() {
        let widths: Vec<i32> = shape
            .tags
            .iter()
            .map(|tag| (tag.chars().count() as f32 * 5.2).ceil() as i32 + 8)
            .collect();
        let total: i32 = widths.iter().sum();
        let tags = shape
            .tags
            .iter()
            .zip(&widths)
            .map(|(tag, w)| {
                Node::new(
                    Place::Sized { w: *w, h: 14 },
                    Style::default()
                        .border_all(Edge::solid(1.0, RULE))
                        .radius_all(2.0)
                        .flex(false, Align::Center, Align::Center),
                )
                .with_children(vec![Node::text(
                    Text::new(tag.clone(), Family::Mono, 8.0, MUTED)
                        .line_height(12.0)
                        .centered(),
                )])
            })
            .collect();
        children.push(
            Node::absolute(
                Rect::new(r.w - 6 - total, r.h - 4 - 14, total, 14),
                Style::default().flex(false, Align::Center, Align::End),
            )
            .with_children(tags),
        );
    }
    // A still diff outlines what changed, outside the box.
    if matches!(marking, Marking::Still | Marking::Ghost) && status != Status::Same {
        let (color, dashed) = match status {
            Status::Added => (ADDED, false),
            Status::Removed => (REMOVED, true),
            _ => (CHANGED, true),
        };
        let edge = if dashed {
            Edge::dashed(2.0, color)
        } else {
            Edge::solid(2.0, color)
        };
        children.push(Node::absolute(
            Rect::new(-4, -4, r.w + 8, r.h + 8),
            Style::default().border_all(edge).radius_all(style.radius[0] + 4.0),
        ));
    }
    Node::absolute(r, style).with_children(children)
}

fn region_node(shape: &RegionShape) -> Node {
    let r = shape.rect;
    let mut style = Style::default();
    let mut label_color = MUTED;
    let mut children = Vec::new();
    match shape.kind {
        RegionKind::Zone => {
            let edge = if shape.dashed {
                Edge::dashed(1.0, MUTED)
            } else {
                Edge::solid(1.0, RULE)
            };
            style = style.border_all(edge).radius_all(10.0);
        }
        RegionKind::Level => {
            style = style.border_all(Edge::solid(1.0, MUTED)).radius_all(10.0);
            if shape.focal {
                style = style.border_all(Edge::solid(1.5, ACCENT)).background(ACCENT_TINT);
                label_color = ACCENT;
            }
        }
        RegionKind::Lane => {
            style.border[0] = Some(Edge::solid(1.0, RULE));
            style.border[2] = Some(Edge::solid(1.0, RULE));
        }
        RegionKind::Column => {
            if shape.focal {
                style = style.background(ACCENT.alpha(0.06));
                label_color = ACCENT;
            }
        }
        RegionKind::Fragment => style = style.border_all(Edge::solid(1.0, MUTED)).radius_all(4.0),
        RegionKind::Activation => style = style.background(WHITE).border_all(Edge::solid(1.0, INK)),
        RegionKind::Lifeline => {
            style.border[3] = Some(Edge::dashed(1.0, RULE));
            return Node::absolute(Rect::new(r.x, r.y, 1, r.h), style);
        }
    }
    if let Some(label) = &shape.label {
        let text = if shape.kind == RegionKind::Lane {
            label.clone()
        } else {
            label.to_uppercase()
        };
        let label_node = match shape.kind {
            RegionKind::Lane => {
                let mut texts = vec![Node::text(
                    Text::new(text, Family::Sans, 12.0, INK).weight(600).line_height(16.0),
                )];
                if let Some(sub) = &shape.sub {
                    texts.push(Node::text(mono(sub, 9.0, MUTED)));
                }
                Node::absolute(
                    Rect::new(12, 12, r.w.min(160), 32),
                    Style::default().flex(true, Align::Start, Align::Start),
                )
                .with_children(texts)
            }
            RegionKind::Column => Node::absolute(
                Rect::new(0, 6, r.w, 14),
                Style::default().flex(false, Align::Center, Align::Center),
            )
            .with_children(vec![Node::text(mono(&text, 9.0, label_color).spacing(0.54))]),
            RegionKind::Fragment => {
                let w = geometry::snap((text.chars().count() as f32 * 6.0) as i32 + 14);
                Node::absolute(
                    Rect::new(0, 0, w, 16),
                    Style {
                        background: Some(PAPER),
                        border: [None, Some(Edge::solid(1.0, MUTED)), Some(Edge::solid(1.0, MUTED)), None],
                        radius: [4.0, 0.0, 4.0, 0.0],
                        ..Style::default()
                    }
                    .flex(false, Align::Center, Align::Center),
                )
                .with_children(vec![Node::text(mono(&text, 9.0, INK).spacing(0.54))])
            }
            _ => {
                let mut texts = vec![Node::text(mono(&text, 9.0, label_color).spacing(0.54))];
                if let Some(sub) = &shape.sub {
                    texts.push(Node {
                        style: Style {
                            padding: [0, 0, 0, 8],
                            ..Style::default()
                        },
                        ..Node::text(mono(sub, 9.0, label_color.alpha(0.8)).spacing(0.54))
                    });
                }
                Node::absolute(
                    Rect::new(10, 6, (r.w - 20).max(0), 14),
                    Style::default().flex(false, Align::Center, Align::Start),
                )
                .with_children(texts)
            }
        };
        children.push(label_node);
    }
    Node::absolute(r, style).with_children(children)
}

fn dot(shape: &scene::DotShape) -> Node {
    let rect = Rect::new(shape.center.x - 8, shape.center.y - 8, 16, 16);
    if shape.ring {
        Node::absolute(
            rect,
            Style::default().border_all(Edge::solid(2.0, INK)).radius_all(8.0).flex(
                false,
                Align::Center,
                Align::Center,
            ),
        )
        .with_children(vec![Node::new(
            Place::Sized { w: 6, h: 6 },
            Style::default().background(INK).radius_all(3.0),
        )])
    } else {
        Node::absolute(rect, Style::default().flex(false, Align::Center, Align::Center)).with_children(vec![Node::new(
            Place::Sized { w: 12, h: 12 },
            Style::default().background(INK).radius_all(6.0),
        )])
    }
}

/// A line, drawn with boxes: each straight stretch a box with one border,
/// each bend a box with two borders and one rounded corner, and the
/// arrowhead a box whose borders meet in a triangle, or two borders turned
/// to a point.
fn line_node(shape: &LineShape, status: Status, marking: &Marking) -> Node {
    let bounds = Shape::Line(shape.clone()).bounds().grow(8);
    let (ox, oy) = (bounds.x, bounds.y);
    let still = matches!(marking, Marking::Still | Marking::Ghost);
    let color = match (status, still) {
        (Status::Removed, true) => REMOVED,
        (Status::Added, true) => ADDED,
        _ => match shape.tone {
            Tone::Ink => INK,
            Tone::Muted => MUTED,
            Tone::Link => LINK,
            Tone::Accent => ACCENT,
        },
    };
    let width: i32 = if shape.tone == Tone::Accent { 2 } else { 1 };
    let edge = if shape.dashed {
        Edge::dashed(width as f32, color)
    } else {
        Edge::solid(width as f32, color)
    };
    let points: Vec<Point> = shape.points.iter().map(|p| Point::new(p.x - ox, p.y - oy)).collect();
    let mut children = stroke(&points, width, edge, shape.head != Head::None);
    match shape.head {
        Head::Arrow => children.push(arrow(&points, color)),
        Head::Open => children.push(chevron(&points, width, color)),
        Head::None => {}
    }
    let mark_at = match &shape.label {
        Some(label) => {
            let rect = label.rect().translate(-ox, -oy);
            let text_color = if shape.tone == Tone::Accent { ACCENT } else { MUTED };
            children.push(
                Node::absolute(
                    rect,
                    Style::default()
                        .background(PAPER)
                        .flex(false, Align::Center, Align::Center),
                )
                .with_children(vec![Node::text(
                    mono(&label.text, 9.0, text_color).line_height(16.0).centered(),
                )]),
            );
            Point::new(rect.right(), rect.y)
        }
        None => {
            let at = geometry::label_point(&shape.points);
            Point::new(at.x - ox + 4, at.y - oy - 8)
        }
    };
    if status != Status::Same && !matches!(marking, Marking::None) {
        let track = match marking {
            Marking::Animated { track } => *track,
            _ => None,
        };
        children.push(badge(
            status,
            18,
            Place::Absolute(Rect::new(mark_at.x - 4, mark_at.y - 8, 18, 18)),
            track,
        ));
    }
    Node::absolute(bounds, Style::default()).with_children(children)
}

/// The stretches and bends of a route through `points`, already relative
/// to its node.
fn stroke(points: &[Point], width: i32, edge: Edge, headed: bool) -> Vec<Node> {
    let o = width / 2;
    // How far each bend is rounded: a quarter circle of RADIUS, or less
    // where a stretch beside it is short.
    let radius: Vec<i32> = (0..points.len())
        .map(|i| {
            if i == 0 || i + 1 == points.len() {
                return 0;
            }
            let before = (points[i].x - points[i - 1].x).abs() + (points[i].y - points[i - 1].y).abs();
            let after = (points[i + 1].x - points[i].x).abs() + (points[i + 1].y - points[i].y).abs();
            geometry::RADIUS.min(before / 2).min(after / 2)
        })
        .collect();
    let mut nodes = Vec::new();
    for i in 0..points.len() - 1 {
        let (a, b) = (points[i], points[i + 1]);
        let (sx, sy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
        // Each end cut back by its bend, and the last by the arrowhead.
        let mut end_cut = radius[i + 1];
        if headed && i + 2 == points.len() {
            end_cut = 4;
        }
        let a = Point::new(a.x + sx * radius[i], a.y + sy * radius[i]);
        let b = Point::new(b.x - sx * end_cut, b.y - sy * end_cut);
        if a.y == b.y && a.x != b.x {
            let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
            let mut style = Style::default();
            style.border[0] = Some(edge);
            nodes.push(Node::absolute(Rect::new(x0, a.y - o, x1 - x0, width), style));
        } else if a.x == b.x && a.y != b.y {
            let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
            let mut style = Style::default();
            style.border[3] = Some(edge);
            nodes.push(Node::absolute(Rect::new(a.x - o, y0, width, y1 - y0), style));
        }
    }
    for i in 1..points.len().saturating_sub(1) {
        let r = radius[i];
        if r == 0 {
            continue;
        }
        let (p, c, n) = (points[i - 1], points[i], points[i + 1]);
        let din = ((c.x - p.x).signum(), (c.y - p.y).signum());
        let dout = ((n.x - c.x).signum(), (n.y - c.y).signum());
        let from = Point::new(c.x - din.0 * r, c.y - din.1 * r);
        let to = Point::new(c.x + dout.0 * r, c.y + dout.1 * r);
        let (x0, y0) = (from.x.min(to.x), from.y.min(to.y));
        let left = c.x == x0;
        let top = c.y == y0;
        let mut style = Style::default();
        // Top, right, bottom, left; and the corner at the bend.
        style.border[if top { 0 } else { 2 }] = Some(edge);
        style.border[if left { 3 } else { 1 }] = Some(edge);
        let corner = match (top, left) {
            (true, true) => 0,
            (true, false) => 1,
            (false, false) => 2,
            (false, true) => 3,
        };
        style.radius[corner] = r as f32 + 0.5;
        nodes.push(Node::absolute(Rect::new(x0 - o, y0 - o, r + width, r + width), style));
    }
    nodes
}

/// A filled arrowhead with its tip on the last point: a box whose one
/// coloured border meets two clear ones in a triangle.
fn arrow(points: &[Point], color: Rgba) -> Node {
    let tip = points[points.len() - 1];
    let before = points[points.len() - 2];
    let (dx, dy) = ((tip.x - before.x).signum(), (tip.y - before.y).signum());
    let solid = Some(Edge::solid(8.0, color));
    let clear = Some(Edge::solid(4.0, Rgba::TRANSPARENT));
    let (rect, border) = match (dx, dy) {
        (0, 1) => (Rect::new(tip.x - 4, tip.y - 8, 8, 8), [solid, clear, None, clear]),
        (0, _) => (Rect::new(tip.x - 4, tip.y, 8, 8), [None, clear, solid, clear]),
        (1, _) => (Rect::new(tip.x - 8, tip.y - 4, 8, 8), [clear, None, clear, solid]),
        _ => (Rect::new(tip.x, tip.y - 4, 8, 8), [clear, solid, clear, None]),
    };
    Node::absolute(
        rect,
        Style {
            border,
            ..Style::default()
        },
    )
}

/// An open arrowhead: the right and bottom borders of a small square,
/// turned so their corner points along the last stretch, onto the tip.
fn chevron(points: &[Point], width: i32, color: Rgba) -> Node {
    let tip = points[points.len() - 1];
    let before = points[points.len() - 2];
    let (dx, dy) = ((tip.x - before.x).signum(), (tip.y - before.y).signum());
    let degrees = match (dx, dy) {
        (0, 1) => 45.0,
        (1, _) => -45.0,
        (0, _) => -135.0,
        _ => 135.0,
    };
    // The corner of a square of 6 is 4 from its centre once turned.
    let (cx, cy) = (tip.x - dx * 4, tip.y - dy * 4);
    let edge = Some(Edge::solid(width as f32, color));
    Node::absolute(
        Rect::new(cx - 3, cy - 3, 6, 6),
        Style {
            border: [None, edge, edge, None],
            rotate: Some(degrees),
            ..Style::default()
        },
    )
}

/// Whether a removed element sits where an element of the second version
/// is drawn, so that drawing it faded would only muddle both.
fn overlaps_second(diff: &Diff, removed: &Element) -> bool {
    if matches!(removed.shape, Shape::Line(_)) {
        return false;
    }
    let bounds = removed.shape.bounds();
    diff.elements.iter().any(|item| {
        !matches!(item.role, Role::Old | Role::Removed) && !matches!(item.element.shape, Shape::Region(_)) && {
            let other = item.element.shape.bounds();
            other.x < bounds.right()
                && bounds.x < other.right()
                && other.y < bounds.bottom()
                && bounds.y < other.bottom()
        }
    })
}

/// Every change, a line each kind: what was added, removed and changed, by
/// what each element shows.
fn ledger(diff: &Diff) -> Vec<(Status, String)> {
    let mut lines = Vec::new();
    for (status, word) in [
        (Status::Added, "ADDED"),
        (Status::Removed, "REMOVED"),
        (Status::Changed, "CHANGED"),
    ] {
        let names: Vec<String> = diff
            .elements
            .iter()
            .filter(|item| item.role != Role::Old && item.element.status == status)
            .map(|item| shown(&item.element))
            .collect();
        if !names.is_empty() {
            lines.push((status, format!("{word}: {}", names.join(" · "))));
        }
    }
    lines
}

/// What an element is called in the ledger: a box by its name, a line by
/// its ends.
fn shown(element: &Element) -> String {
    match &element.shape {
        Shape::Box(shape) if !shape.name.is_empty() => shape.name.clone(),
        Shape::Region(RegionShape { label: Some(label), .. }) => label.clone(),
        Shape::Text(shape) => shape.text.clone(),
        _ => {
            let id = element.id.split_once(':').map_or(element.id.as_str(), |(_, rest)| rest);
            id.replace("->", " → ")
        }
    }
}
