//! A parent and its children. The root is at the top, or at the left when
//! the flow runs right. Leaves sit side by side, and a parent is centred
//! over its children. A line runs as a bus: from the parent, across over
//! the children, and into each.

use diagram_format::{Branch, Flow, Tree};

use super::{Canvas, line_shape, uniform_height, uniform_width};
use crate::geometry::snap;
use crate::scene::*;

const GAP_SIBLING: i32 = 24;
const GAP_LEVEL: i32 = 56;

pub fn layout(body: &Tree, canvas: &mut Canvas) {
    let all = body.root.walk();
    let items: Vec<_> = all.iter().map(|branch| &branch.item).collect();
    let w = uniform_width(items.iter().copied());
    let h = uniform_height(items.iter().copied());
    let down = body.flow == Flow::Down;
    // Along the levels, and across them: a level is a row when the flow
    // runs down, and a column when it runs right.
    let (slot, level) = if down {
        (w + GAP_SIBLING, h + GAP_LEVEL)
    } else {
        (h + GAP_SIBLING / 2, w + GAP_LEVEL)
    };
    let mut placed = Vec::new();
    let mut next_leaf = 0;
    place(&body.root, 0, &mut next_leaf, &mut placed);
    let rect_of = |across: f32, depth: usize| {
        let minor = snap((across * slot as f32) as i32);
        let major = depth as i32 * level;
        if down {
            Rect::new(minor, major, w, h)
        } else {
            Rect::new(major, minor, w, h)
        }
    };
    for (branch, across, depth) in &placed {
        canvas.item(
            format!("item:{}", branch.item.key),
            2 * *depth as u32,
            &branch.item,
            rect_of(*across, *depth),
        );
    }
    for (branch, across, depth) in &placed {
        let parent = rect_of(*across, *depth);
        for child in &branch.children {
            let (_, child_across, child_depth) = placed
                .iter()
                .find(|(other, _, _)| other.item.key == child.item.key)
                .expect("every branch is placed");
            let child_rect = rect_of(*child_across, *child_depth);
            let points = if down {
                let bus = parent.bottom() + GAP_LEVEL / 2;
                vec![
                    Point::new(parent.cx(), parent.bottom()),
                    Point::new(parent.cx(), bus),
                    Point::new(child_rect.cx(), bus),
                    Point::new(child_rect.cx(), child_rect.y),
                ]
            } else {
                let bus = parent.right() + GAP_LEVEL / 2;
                vec![
                    Point::new(parent.right(), parent.cy()),
                    Point::new(bus, parent.cy()),
                    Point::new(bus, child_rect.cy()),
                    Point::new(child_rect.x, child_rect.cy()),
                ]
            };
            let points = crate::geometry::simplify(points);
            canvas.push(
                format!("line:{}->{}", branch.item.key, child.item.key),
                2 * *child_depth as u32 - 1,
                Shape::Line(line_shape(points, Head::None, false, Tone::Muted, None)),
            );
        }
    }
}

/// Places every branch: a leaf at the next free slot, a parent at the
/// middle of its children. Gives back the slot of `branch`.
fn place<'t>(
    branch: &'t Branch,
    depth: usize,
    next_leaf: &mut usize,
    placed: &mut Vec<(&'t Branch, f32, usize)>,
) -> f32 {
    let across = if branch.children.is_empty() {
        let at = *next_leaf as f32;
        *next_leaf += 1;
        at
    } else {
        let slots: Vec<f32> = branch
            .children
            .iter()
            .map(|child| place(child, depth + 1, next_leaf, placed))
            .collect();
        (slots[0] + slots[slots.len() - 1]) / 2.0
    };
    placed.push((branch, across, depth));
    across
}
