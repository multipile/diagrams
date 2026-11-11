//! What depends on what, in ranks: the boxes nothing depends on first, each
//! dependency a rank further along. A box that several depend on says how
//! many, and a box that depends on nothing is drawn as a store. The one
//! cycle runs around the outside, in the accent.

use std::collections::BTreeMap;

use diagram_format::{Dependency, Flow, Item, LineKind, Look, rules};

use super::{Canvas, item_box, line_shape, uniform_height, uniform_width};
use crate::geometry::{Ports, Side, across, snap};
use crate::scene::*;

const GAP_MINOR: i32 = 32;
const GAP_MAJOR: i32 = 80;

pub fn layout(body: &Dependency, canvas: &mut Canvas) {
    let ranks = rules::ranks(body).unwrap_or_default();
    // A box that several depend on carries a tag saying how many, inside
    // its bottom right corner, where no line meets it.
    let tagged = body
        .boxes
        .iter()
        .any(|item| body.lines.iter().filter(|line| line.to == item.key).count() >= 2);
    let w = uniform_width(&body.boxes) + if tagged { 40 } else { 0 };
    let h = uniform_height(&body.boxes);
    let down = body.flow == Flow::Down;
    let rows = order(body, &ranks);

    let (along, thick) = if down { (w, h) } else { (h, w) };
    let widest = rows
        .iter()
        .map(|row| row.len() as i32 * along + (row.len() as i32 - 1).max(0) * GAP_MINOR)
        .max()
        .unwrap_or(0);
    let mut rects: BTreeMap<&str, Rect> = BTreeMap::new();
    for (rank, row) in rows.iter().enumerate() {
        let length = row.len() as i32 * along + (row.len() as i32 - 1) * GAP_MINOR;
        let start = snap((widest - length) / 2);
        let major = rank as i32 * (thick + GAP_MAJOR);
        for (index, item) in row.iter().enumerate() {
            let minor = start + index as i32 * (along + GAP_MINOR);
            let rect = if down {
                Rect::new(minor, major, w, h)
            } else {
                Rect::new(major, minor, w, h)
            };
            rects.insert(item.key.as_str(), rect);
        }
    }

    let (out_side, in_side) = if down {
        (Side::Bottom, Side::Top)
    } else {
        (Side::Right, Side::Left)
    };
    let mut ports = Ports::default();
    for (index, line) in body.lines.iter().enumerate() {
        let (from, to) = (rects[line.from.as_str()], rects[line.to.as_str()]);
        let (toward_to, toward_from) = if down {
            (to.cx(), from.cx())
        } else {
            (to.cy(), from.cy())
        };
        ports.add(&line.from, out_side, index, toward_to);
        ports.add(&line.to, in_side, index, toward_from);
    }

    for item in &body.boxes {
        let rank = ranks.get(&item.key).copied().unwrap_or(0) as u32;
        let mut shape = item_box(item, rects[item.key.as_str()]);
        let dependents = body.lines.iter().filter(|line| line.to == item.key).count();
        if dependents >= 2 {
            shape.tags = vec![format!("{dependents} IN")];
        }
        let depends = body.lines.iter().any(|line| line.from == item.key);
        if !depends && item.look == Look::Plain {
            shape.look = Look::Store;
        }
        canvas.push(format!("box:{}", item.key), 2 * rank, Shape::Box(shape));
    }

    let all: Vec<Rect> = rects.values().copied().collect();
    let mut channel: BTreeMap<i32, i32> = BTreeMap::new();
    for (index, line) in body.lines.iter().enumerate() {
        let (from, to) = (rects[line.from.as_str()], rects[line.to.as_str()]);
        let length = if down { w } else { h };
        let start = out_side.point(&from, ports.offset(&line.from, out_side, index, length));
        let end = in_side.point(&to, ports.offset(&line.to, in_side, index, length));
        let others: Vec<Rect> = all
            .iter()
            .copied()
            .filter(|rect| *rect != from && *rect != to)
            .collect();
        // Cross in the channel after the box the line leaves, or in the one
        // before the box it enters, whichever passes behind fewer boxes.
        let (after, before) = if down {
            (from.bottom(), to.y)
        } else {
            (from.right(), to.x)
        };
        let slot = channel.entry(after).or_default();
        let offset = 20 + (*slot % 5) * 8;
        *slot += 1;
        let points = across(start, end, down, after + offset, before - offset, &others, 0);
        let rank = ranks.get(&line.to).copied().unwrap_or(1) as u32;
        canvas.line(
            format!("line:{}->{}", line.from, line.to),
            (2 * rank).saturating_sub(1),
            points,
            LineKind::Call,
            line.focal,
            None,
        );
    }

    if let Some(cycle) = &body.cycle {
        let (from, to) = (rects[cycle.from.as_str()], rects[cycle.to.as_str()]);
        let points = if down {
            let outside = all.iter().map(Rect::right).max().unwrap_or(0) + 32;
            vec![
                Side::Right.point(&from, 0),
                Point::new(outside, from.cy()),
                Point::new(outside, to.cy()),
                Side::Right.point(&to, 0),
            ]
        } else {
            let outside = all.iter().map(Rect::bottom).max().unwrap_or(0) + 32;
            vec![
                Side::Bottom.point(&from, 0),
                Point::new(from.cx(), outside),
                Point::new(to.cx(), outside),
                Side::Bottom.point(&to, 0),
            ]
        };
        let last = rows.len() as u32 * 2;
        canvas.push(
            format!("cycle:{}->{}", cycle.from, cycle.to),
            last,
            Shape::Line(line_shape(points, Head::Arrow, false, Tone::Accent, Some("cycle"))),
        );
    }
}

/// The boxes of each rank, each rank ordered so its boxes sit near the
/// boxes that depend on them.
fn order<'d>(body: &'d Dependency, ranks: &BTreeMap<String, usize>) -> Vec<Vec<&'d Item>> {
    let count = ranks.values().max().map_or(0, |rank| rank + 1);
    let mut rows: Vec<Vec<&Item>> = vec![Vec::new(); count];
    for item in &body.boxes {
        rows[ranks.get(&item.key).copied().unwrap_or(0)].push(item);
    }
    for rank in 1..rows.len() {
        let place: BTreeMap<&str, usize> = rows[rank - 1]
            .iter()
            .enumerate()
            .map(|(index, item)| (item.key.as_str(), index))
            .collect();
        let mut keyed: Vec<(usize, f32, &Item)> = rows[rank]
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let above: Vec<usize> = body
                    .lines
                    .iter()
                    .filter(|line| line.to == item.key)
                    .filter_map(|line| place.get(line.from.as_str()).copied())
                    .collect();
                let center = if above.is_empty() {
                    index as f32
                } else {
                    above.iter().sum::<usize>() as f32 / above.len() as f32
                };
                (index, center, *item)
            })
            .collect();
        keyed.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        rows[rank] = keyed.into_iter().map(|(_, _, item)| item).collect();
    }
    rows
}
