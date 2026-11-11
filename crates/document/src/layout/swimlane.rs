//! The steps of one process, each in the lane of whoever does it. Lanes run
//! left to right, one above another. A step's column is how many steps lead
//! to it along the lines, so the flow runs mostly straight. A line from one
//! lane to another is a handoff.

use std::collections::BTreeMap;

use diagram_format::{Item, Swimlane};

use super::{Canvas, uniform_height, uniform_width};
use crate::geometry::{Ports, Side, elbow, name_width, snap};
use crate::scene::*;

const LANE_PAD: i32 = 16;
const GAP_COLUMN: i32 = 56;

pub fn layout(body: &Swimlane, canvas: &mut Canvas) {
    let steps: Vec<(&Item, usize)> = body
        .lanes
        .iter()
        .enumerate()
        .flat_map(|(lane, l)| l.steps.iter().map(move |step| (step, lane)))
        .collect();
    let w = uniform_width(steps.iter().map(|(step, _)| *step));
    let h = uniform_height(steps.iter().map(|(step, _)| *step));
    let columns = columns(body, &steps);
    let count = columns.values().max().map_or(1, |column| column + 1) as i32;
    let label_w = snap(
        body.lanes
            .iter()
            .map(|lane| name_width(&lane.name))
            .max()
            .unwrap_or(0)
            .max(72)
            + 2 * LANE_PAD,
    );
    let lane_h = h + 2 * LANE_PAD;
    let total_w = label_w + LANE_PAD + count * w + (count - 1) * GAP_COLUMN + LANE_PAD;

    let mut rects: BTreeMap<&str, Rect> = BTreeMap::new();
    for (step, lane) in &steps {
        let column = columns[step.key.as_str()] as i32;
        let rect = Rect::new(
            label_w + LANE_PAD + column * (w + GAP_COLUMN),
            *lane as i32 * lane_h + LANE_PAD,
            w,
            h,
        );
        rects.insert(step.key.as_str(), rect);
    }

    for (index, lane) in body.lanes.iter().enumerate() {
        canvas.push(
            format!("lane:{}", lane.key),
            0,
            Shape::Region(RegionShape {
                rect: Rect::new(0, index as i32 * lane_h, total_w, lane_h),
                kind: RegionKind::Lane,
                label: Some(lane.name.clone()),
                sub: None,
                dashed: false,
                focal: false,
            }),
        );
    }

    let lane_of: BTreeMap<&str, usize> = steps.iter().map(|(step, lane)| (step.key.as_str(), *lane)).collect();
    let mut ports = Ports::default();
    let mut sides = Vec::new();
    for (index, line) in body.lines.iter().enumerate() {
        let (from, to) = (rects[line.from.as_str()], rects[line.to.as_str()]);
        let (out, into) = if to.x > from.x {
            (Side::Right, Side::Left)
        } else if to.x == from.x {
            if to.y > from.y {
                (Side::Bottom, Side::Top)
            } else {
                (Side::Top, Side::Bottom)
            }
        } else {
            (Side::Top, Side::Top)
        };
        ports.add(&line.from, out, index, if out.vertical() { to.cx() } else { to.cy() });
        ports.add(
            &line.to,
            into,
            index,
            if into.vertical() { from.cx() } else { from.cy() },
        );
        sides.push((out, into));
    }
    for (index, (line, (out, into))) in body.lines.iter().zip(sides).enumerate() {
        let (from, to) = (rects[line.from.as_str()], rects[line.to.as_str()]);
        let length = |rect: &Rect, side: Side| if side.vertical() { rect.w } else { rect.h };
        let start = out.point(&from, ports.offset(&line.from, out, index, length(&from, out)));
        let end = into.point(&to, ports.offset(&line.to, into, index, length(&to, into)));
        let points = match (out, into) {
            (Side::Right, Side::Left) => elbow(start, end, false, from.right() + GAP_COLUMN / 2),
            (Side::Top, Side::Top) => {
                // Back to an earlier column: over the top of the lane.
                let lane_top = lane_of[line.from.as_str()].min(lane_of[line.to.as_str()]) as i32 * lane_h;
                let y = lane_top + LANE_PAD / 2;
                vec![start, Point::new(start.x, y), Point::new(end.x, y), end]
            }
            _ => elbow(start, end, true, (start.y + end.y) / 2),
        };
        let column = columns[line.from.as_str()] as u32;
        canvas.line(
            format!("line:{}->{}", line.from, line.to),
            2 * column + 2,
            crate::geometry::simplify(points),
            line.kind,
            line.focal,
            line.label.as_deref(),
        );
    }

    for (step, _) in &steps {
        let column = columns[step.key.as_str()] as u32;
        canvas.item(
            format!("step:{}", step.key),
            2 * column + 1,
            step,
            rects[step.key.as_str()],
        );
    }
}

/// Each step's column: how many steps lead to it along the lines, moved on
/// when an earlier step of its lane already holds that column. A line that
/// closes a loop runs back, and leads nowhere new.
fn columns<'s>(body: &Swimlane, steps: &[(&'s Item, usize)]) -> BTreeMap<&'s str, usize> {
    let keys: Vec<&str> = steps.iter().map(|(step, _)| step.key.as_str()).collect();
    let back = back_lines(body, &keys);
    let mut column: BTreeMap<&str, usize> = keys.iter().map(|key| (*key, 0)).collect();
    for _ in 0..keys.len() {
        let mut changed = false;
        for (n, line) in body.lines.iter().enumerate() {
            if back.contains(&n) {
                continue;
            }
            let (Some(&from), Some(&to)) = (column.get(line.from.as_str()), column.get(line.to.as_str())) else {
                continue;
            };
            if to < from + 1 {
                let key = keys.iter().find(|key| **key == line.to).copied().expect("a step's key");
                column.insert(key, from + 1);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let lanes = steps.iter().map(|(_, lane)| *lane).max().map_or(0, |lane| lane + 1);
    for lane in 0..lanes {
        let mut taken: Vec<usize> = Vec::new();
        let mut in_lane: Vec<&str> = steps
            .iter()
            .filter(|(_, l)| *l == lane)
            .map(|(step, _)| step.key.as_str())
            .collect();
        in_lane.sort_by_key(|key| column[key]);
        for key in in_lane {
            let mut at = column[key];
            while taken.contains(&at) {
                at += 1;
            }
            taken.push(at);
            column.insert(key, at);
        }
    }
    column
}

/// The lines that close a loop: those a depth-first walk from the steps,
/// in the order written, finds running back to a step still on its path.
fn back_lines(body: &Swimlane, keys: &[&str]) -> Vec<usize> {
    fn visit(at: &str, body: &Swimlane, path: &mut Vec<String>, done: &mut Vec<String>, back: &mut Vec<usize>) {
        path.push(at.to_string());
        for (n, line) in body.lines.iter().enumerate() {
            if line.from != at {
                continue;
            }
            if path.contains(&line.to) {
                back.push(n);
            } else if !done.contains(&line.to) {
                visit(&line.to, body, path, done, back);
            }
        }
        path.pop();
        done.push(at.to_string());
    }
    let mut back = Vec::new();
    let mut done = Vec::new();
    let targets: Vec<&str> = body.lines.iter().map(|line| line.to.as_str()).collect();
    // Start from the steps nothing leads to, then from any left.
    let mut starts: Vec<&str> = keys.iter().copied().filter(|key| !targets.contains(key)).collect();
    starts.extend(keys.iter().copied());
    for key in starts {
        if !done.iter().any(|d| d == key) {
            visit(key, body, &mut Vec::new(), &mut done, &mut back);
        }
    }
    back
}
