//! Scopes inside scopes. Each level is drawn inside the one before, with
//! the same space between each pair, and its label at its top left.

use diagram_format::Nested;

use super::Canvas;
use crate::geometry::snap;
use crate::scene::*;

const PAD: i32 = 20;
const LABEL_H: i32 = 32;

pub fn layout(body: &Nested, canvas: &mut Canvas) {
    let count = body.levels.len() as i32;
    // The innermost level is wide enough for its own label; every level
    // outside it adds the same space on each side.
    let label = |index: usize| {
        let level = &body.levels[index];
        // The label is drawn in monospace capitals at 9px, with its sub
        // beside it.
        let chars = level.name.chars().count() + 1 + level.sub.as_deref().map_or(0, |sub| sub.chars().count() + 2);
        (chars as f32 * 6.4).ceil() as i32 + 2 * PAD
    };
    let mut inner_w = snap(label(body.levels.len() - 1).max(160));
    for (index, _) in body.levels.iter().enumerate() {
        let depth = count - 1 - index as i32;
        inner_w = inner_w.max(snap(label(index) - 2 * PAD * depth));
    }
    let inner_h = 56;
    for (index, level) in body.levels.iter().enumerate() {
        let depth = index as i32;
        let inside = count - 1 - depth;
        let rect = Rect::new(
            depth * PAD,
            depth * LABEL_H,
            inner_w + 2 * PAD * inside,
            inner_h + (LABEL_H + PAD) * inside,
        );
        let mut name = level.name.clone();
        if level.pile.is_some() && !name.starts_with(":/") {
            name = format!("@{name}");
        }
        canvas.push(
            format!("level:{}", level.key),
            index as u32,
            Shape::Region(RegionShape {
                rect,
                kind: RegionKind::Level,
                label: Some(name),
                sub: level.sub.clone(),
                dashed: false,
                focal: level.focal,
            }),
        );
    }
}
