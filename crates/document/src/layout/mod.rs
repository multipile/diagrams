//! One layout for each type of diagram.
//!
//! A layout places every element of a diagram by its type's rules: ranks
//! for a dependency diagram, lanes and columns for a process, a row of
//! lifelines for a sequence. The diagram never says where anything goes.
//! Each layout draws on a [`Canvas`] from the top left, and the canvas
//! makes room for the heading and the note when it is done.

mod architecture;
mod dependency;
mod nested;
mod process;
mod sequence;
mod state;
mod swimlane;
mod tree;
mod treemap;

use std::collections::BTreeSet;

use diagram_format::{Body, Diagram, Frame, Item, LineKind};

use crate::geometry::{self, MARGIN};
use crate::scene::*;

/// Where the heading is drawn, above everything else.
pub const HEADING_H: i32 = 48;
/// Where the note is drawn, below everything else.
pub const NOTE_H: i32 = 32;

/// Lays a diagram out.
pub fn layout(diagram: &Diagram) -> Scene {
    let mut canvas = Canvas::default();
    match &diagram.body {
        Body::Architecture(body) => architecture::layout(body, &mut canvas),
        Body::Sequence(body) => sequence::layout(body, &mut canvas),
        Body::State(body) => state::layout(body, &mut canvas),
        Body::Swimlane(body) => swimlane::layout(body, &mut canvas),
        Body::Process(body) => process::layout(body, &mut canvas),
        Body::Tree(body) => tree::layout(body, &mut canvas),
        Body::Dependency(body) => dependency::layout(body, &mut canvas),
        Body::Nested(body) => nested::layout(body, &mut canvas),
        Body::Treemap(body) => treemap::layout(body, &mut canvas),
    }
    canvas.finish(&diagram.frame)
}

/// The elements of a scene as a layout places them.
#[derive(Debug, Default)]
pub struct Canvas {
    elements: Vec<Element>,
    ids: BTreeSet<String>,
}

impl Canvas {
    /// Places a shape. A second element with the same id is given `#2`.
    pub fn push(&mut self, id: impl Into<String>, step: u32, shape: Shape) {
        let id = id.into();
        let mut unique = id.clone();
        let mut n = 2;
        while !self.ids.insert(unique.clone()) {
            unique = format!("{id}#{n}");
            n += 1;
        }
        self.elements.push(Element {
            id: unique,
            step,
            status: Status::Same,
            shape,
        });
    }

    /// A box for an item.
    pub fn item(&mut self, id: impl Into<String>, step: u32, item: &Item, rect: Rect) {
        self.push(id, step, Shape::Box(item_box(item, rect)));
    }

    /// A line along `points`, drawn as `kind` says.
    pub fn line(
        &mut self,
        id: impl Into<String>,
        step: u32,
        points: Vec<Point>,
        kind: LineKind,
        focal: bool,
        label: Option<&str>,
    ) {
        let (dashed, head, tone) = match kind {
            LineKind::Call => (false, Head::Arrow, Tone::Muted),
            LineKind::Link => (false, Head::Arrow, Tone::Link),
            LineKind::Async => (true, Head::Open, Tone::Muted),
            LineKind::Optional => (true, Head::Arrow, Tone::Muted),
        };
        let tone = if focal { Tone::Accent } else { tone };
        self.push(id, step, Shape::Line(line_shape(points, head, dashed, tone, label)));
    }

    /// The scene: everything moved inside the margin, below the heading and
    /// above the note.
    pub fn finish(mut self, frame: &Frame) -> Scene {
        let bounds = self
            .elements
            .iter()
            .map(|element| element.shape.bounds())
            .reduce(|a, b| a.union(&b))
            .unwrap_or_default();
        let dx = MARGIN - bounds.x;
        let dy = MARGIN + HEADING_H - bounds.y;
        for element in &mut self.elements {
            element.shape.translate(dx, dy);
        }
        let heading = geometry::snap((frame.title.chars().count() as f32 * 9.6) as i32);
        let note = frame
            .note
            .as_deref()
            .map_or(0, |note| geometry::snap((note.chars().count() as f32 * 6.4) as i32));
        let width = geometry::snap(bounds.w.max(heading).max(note) + 2 * MARGIN);
        let note_h = if frame.note.is_some() { NOTE_H } else { 0 };
        let height = geometry::snap(MARGIN + HEADING_H + bounds.h + note_h + MARGIN);
        Scene {
            width,
            height,
            title: frame.title.clone(),
            desc: frame.desc.clone(),
            note: frame.note.clone(),
            elements: self.elements,
        }
    }
}

pub fn item_box(item: &Item, rect: Rect) -> BoxShape {
    let mut shape = BoxShape::new(rect, item.name.clone());
    shape.sub = item.sub.clone();
    shape.look = item.look;
    shape.focal = item.focal;
    shape.pile = item.pile.as_ref().map(|pile| pile.reference.clone());
    shape
}

pub fn line_shape(points: Vec<Point>, head: Head, dashed: bool, tone: Tone, label: Option<&str>) -> LineShape {
    let label = label.map(|text| Label {
        text: text.to_uppercase(),
        at: geometry::label_point(&points),
    });
    LineShape {
        points,
        head,
        dashed,
        tone,
        label,
    }
}

/// The width every box of a layout is drawn at: the widest one's.
pub fn uniform_width<'i>(items: impl IntoIterator<Item = &'i Item>) -> i32 {
    items
        .into_iter()
        .map(|item| geometry::box_width(&item.name, item.sub.as_deref(), None))
        .max()
        .unwrap_or(geometry::BOX_MIN_W)
}

/// The height every box of a layout is drawn at: tall enough for a second
/// line when any box has one.
pub fn uniform_height<'i>(items: impl IntoIterator<Item = &'i Item>) -> i32 {
    let sub = items.into_iter().any(|item| item.sub.is_some());
    geometry::box_height(sub, false)
}
