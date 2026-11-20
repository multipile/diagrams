//! What depends on what, in ranks: the boxes nothing depends on first, each
//! dependency a rank further along, and the boxes of a rank in the order the
//! file writes them. A box that several depend on says how
//! many, and a box that depends on nothing is drawn as a store. The one
//! cycle runs around the outside, in the accent.

use std::collections::BTreeMap;

use diagram_format::{Dependency, Flow, Item, Look, rules};

use super::channel::{Turn, channel_order, fits, wanted};
use super::{Canvas, item_box, line_shape, uniform_height, uniform_width};
use crate::geometry::{GRID, Ports, Side, across, simplify, snap};
use crate::scene::*;

const GAP_MINOR: i32 = 32;
const GAP_MAJOR: i32 = 80;
/// How far past a rank a line first turns, and how far apart the turns in
/// one channel are at most.
const TURN: i32 = 20;
const PITCH: i32 = 8;
/// How many times the ports may move before the routes are kept.
const ROUNDS: usize = 32;

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
    let all: Vec<Rect> = rects.values().copied().collect();
    let others = |from: Rect, to: Rect| -> Vec<Rect> {
        all.iter()
            .copied()
            .filter(|rect| *rect != from && *rect != to)
            .collect()
    };
    // The ends that meet one side of a box are ordered by where each line
    // heads from there, not by where the box at its other end is: a line
    // that goes round the boxes between leaves on the side of its lane, and
    // enters on that side, so that it crosses no line beside it. Where it
    // heads is learned by routing it once from middle to middle.
    let mut ports = Ports::default();
    for (index, line) in body.lines.iter().enumerate() {
        let (from, to) = (rects[line.from.as_str()], rects[line.to.as_str()]);
        let (after, before) = if down {
            (from.bottom(), to.y)
        } else {
            (from.right(), to.x)
        };
        let route = across(
            out_side.point(&from, 0),
            in_side.point(&to, 0),
            down,
            after + TURN,
            before - TURN,
            &others(from, to),
            0,
        );
        let (leaving, entering) = heading(&route, down);
        ports.add(&line.from, out_side, index, leaving);
        ports.add(&line.to, in_side, index, entering);
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

    // Each line crosses in the channel after the box it leaves, or in the
    // one before the box it enters, whichever passes behind fewer boxes, and
    // runs past the ranks between along a lane beside them. Every turn in a
    // channel is noted, and then each channel's turns are put at their own
    // places, in the order that has the lines meet the fewest times. When a
    // stretch still lines up with another's, or a turn is too short to
    // round, the port at the end of one moves along its box's side, as in
    // an architecture diagram, and the routes are made again.
    let along = |point: Point| if down { point.y } else { point.x };
    let side = |point: Point| if down { point.x } else { point.y };
    let length = if down { w } else { h };
    let end_of = |index: usize, start: bool| {
        let line = &body.lines[index];
        if start {
            (line.from.as_str(), out_side, length)
        } else {
            (line.to.as_str(), in_side, length)
        }
    };
    let mut nudges: BTreeMap<(usize, bool), i32> = BTreeMap::new();
    let mut round = 0;
    let routes = loop {
        let mut routes: Vec<Vec<Point>> = Vec::new();
        let mut turns: BTreeMap<usize, Vec<Turn>> = BTreeMap::new();
        for (index, line) in body.lines.iter().enumerate() {
            let (from, to) = (rects[line.from.as_str()], rects[line.to.as_str()]);
            let nudge = |start: bool| nudges.get(&(index, start)).copied().unwrap_or(0);
            let start = out_side.point(&from, ports.offset(&line.from, out_side, index, length) + nudge(true));
            let end = in_side.point(&to, ports.offset(&line.to, in_side, index, length) + nudge(false));
            let (after, before) = if down {
                (from.bottom(), to.y)
            } else {
                (from.right(), to.x)
            };
            let (leave, arrive) = (after + TURN, before - TURN);
            let route = across(start, end, down, leave, arrive, &others(from, to), 0);
            let rank_of = |key: &str| ranks.get(key).copied().unwrap_or(0);
            let (leaving, arriving) = (rank_of(&line.from), rank_of(&line.to).saturating_sub(1));
            let last = route.len() - 2;
            for (segment, pair) in route.windows(2).enumerate() {
                let (p, q) = (pair[0], pair[1]);
                if along(p) != along(q) {
                    continue;
                }
                let channel = if along(p) == leave {
                    leaving
                } else if along(p) == arrive {
                    arriving
                } else {
                    continue;
                };
                // The stretch before the turn ends at the line's start when
                // it is the first, and the one after at its end when it is
                // the last.
                turns.entry(channel).or_default().push(Turn {
                    line: index,
                    segment,
                    near: side(p),
                    far: side(q),
                    near_end: (segment == 1).then_some(true),
                    far_end: (segment + 1 == last).then_some(false),
                    back: false,
                });
            }
            routes.push(route);
        }
        let mut moved = false;
        for (channel, here) in &turns {
            let far = *channel as i32 * (thick + GAP_MAJOR) + thick;
            let n = here.len() as i32;
            let pitch = if n > 1 {
                ((GAP_MAJOR - 2 * TURN) / (n - 1)).div_euclid(GRID) * GRID
            } else {
                PITCH
            }
            .clamp(GRID, PITCH);
            let order = channel_order(here);
            for (k, turn) in order.iter().map(|t| &here[*t]).enumerate() {
                let at = far + TURN + k as i32 * pitch;
                for point in &mut routes[turn.line][turn.segment..=turn.segment + 1] {
                    if down {
                        point.y = at;
                    } else {
                        point.x = at;
                    }
                }
            }
            if moved || round >= ROUNDS {
                continue;
            }
            if let Some((end, by)) = wanted(here, &order).into_iter().find_map(|(line, end, by)| {
                let start = end?;
                fits(&ports, &nudges, &end_of, line, start, by).then_some(((line, start), by))
            }) {
                *nudges.entry(end).or_default() += by;
                moved = true;
            }
        }
        round += 1;
        if !moved {
            break routes;
        }
    };
    for (line, points) in body.lines.iter().zip(routes) {
        let rank = ranks.get(&line.to).copied().unwrap_or(1) as u32;
        canvas.written_line(
            format!("line:{}->{}", line.from, line.to),
            (2 * rank).saturating_sub(1),
            simplify(points),
            line,
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
        let mut shape = line_shape(points, Head::Arrow, false, Tone::Accent, Some("cycle"));
        shape.ends = Some((cycle.from.clone(), cycle.to.clone()));
        shape.meta = metadata(&cycle.meta);
        canvas.push(format!("cycle:{}->{}", cycle.from, cycle.to), last, Shape::Line(shape));
    }
}

/// Where a route heads, across the flow, from each of its ends: the first
/// place it turns to after leaving its start, and the last it comes from
/// before entering its end. A straight route heads where it is.
fn heading(route: &[Point], down: bool) -> (i32, i32) {
    let across = |point: &Point| if down { point.x } else { point.y };
    let (first, last) = (across(&route[0]), across(&route[route.len() - 1]));
    let leaving = route.iter().map(across).find(|&at| at != first).unwrap_or(first);
    let entering = route.iter().rev().map(across).find(|&at| at != last).unwrap_or(last);
    (leaving, entering)
}

/// The boxes of each rank, in the order the file writes them: where a box
/// sits across the flow is the file's to say. When another order would
/// have fewer lines cross, `diagram check` hints it.
fn order<'d>(body: &'d Dependency, ranks: &BTreeMap<String, usize>) -> Vec<Vec<&'d Item>> {
    let count = ranks.values().max().map_or(0, |rank| rank + 1);
    let mut rows: Vec<Vec<&Item>> = vec![Vec::new(); count];
    for item in &body.boxes {
        rows[ranks.get(&item.key).copied().unwrap_or(0)].push(item);
    }
    rows
}
