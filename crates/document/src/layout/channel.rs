//! The turns of the lines that cross one channel, the space between two
//! ranks or tiers, and the order they turn in so that they meet the fewest
//! times. Architecture and dependency diagrams route their lines through
//! channels, and both order their turns this way.

use std::collections::BTreeMap;

use crate::geometry::{Ports, RADIUS, Side, meetings};
use crate::scene::Point;

/// How far a port moves along its side to keep its stretch out of line with
/// another's, and how far it stays from the side's ends and other ports.
pub const STEP: i32 = 12;

/// Where a line turns in a channel: the segment of its route that crosses
/// the channel, from the side `near` the tier before it to the side `far`
/// toward the tier after, across the flow. A line that comes `back` to the
/// tier it left has both its ends on the near side.
pub struct Turn {
    pub line: usize,
    pub segment: usize,
    pub near: i32,
    pub far: i32,
    /// Which end of the line, its start or its end, the stretch on each
    /// side of the turn reaches, if it reaches one.
    pub near_end: Option<bool>,
    pub far_end: Option<bool>,
    pub back: bool,
}

impl Turn {
    /// The turn at `at` along the channel, with its stretches running out
    /// of the channel on either side, flow running right.
    fn sketch(&self, at: i32) -> Vec<Point> {
        const OUT: i32 = 1 << 20;
        let end = if self.back { -OUT } else { OUT };
        vec![
            Point::new(-OUT, self.near),
            Point::new(at, self.near),
            Point::new(at, self.far),
            Point::new(end, self.far),
        ]
    }
}

/// The order of the turns in one channel, the first nearest the tier
/// before it, that has the lines meet the fewest times. A line that comes
/// back goes inside the lines that cross; of two lines that cross toward
/// the same side, the one starting further that way turns first. Then any
/// move of one turn that makes fewer meetings is taken.
pub fn channel_order(turns: &[Turn]) -> Vec<usize> {
    let n = turns.len();
    let cost: Vec<Vec<usize>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    if i == j {
                        0
                    } else {
                        meetings(&turns[i].sketch(1), &turns[j].sketch(2))
                    }
                })
                .collect()
        })
        .collect();
    let total = |order: &[usize]| -> usize {
        (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .map(|(i, j)| cost[order[i]][order[j]])
            .sum()
    };
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&t| {
        let turn = &turns[t];
        if turn.back {
            (0, (turn.far - turn.near).abs(), turn.near)
        } else if turn.far > turn.near {
            (1, -turn.near, -turn.far)
        } else {
            (2, turn.near, turn.far)
        }
    });
    let mut best = total(&order);
    loop {
        let mut moved = false;
        for from in 0..n {
            for to in 0..n {
                let mut tried = order.clone();
                let turn = tried.remove(from);
                tried.insert(to, turn);
                let cost = total(&tried);
                if cost < best {
                    (best, order, moved) = (cost, tried, true);
                }
            }
        }
        if !moved {
            return order;
        }
    }
}

/// The moves of a port, each a line, which of its ends (its start or its
/// end), and how far along its side, that would mend the turns of one
/// channel, in `order`, best first. First a stretch that lines up with
/// another's, running into it or on from where it stops: one of their ends
/// moves a step along its side. Then a turn too short to round: the port it
/// leads to moves in line with the stretch before it, so the line runs
/// straight.
pub fn wanted(here: &[Turn], order: &[usize]) -> Vec<(usize, Option<bool>, i32)> {
    let mut wanted: Vec<(usize, Option<bool>, i32)> = Vec::new();
    for (k, i) in order.iter().map(|t| &here[*t]).enumerate() {
        for j in order[k + 1..].iter().map(|t| &here[*t]) {
            if i.back || j.back {
                continue;
            }
            if i.near == j.far {
                wanted.extend(steps(j.line, j.far_end, j.near - j.far));
                wanted.extend(steps(i.line, i.near_end, i.far - i.near));
            }
            if i.far == j.near {
                wanted.extend(steps(i.line, i.far_end, i.near - i.far));
                wanted.extend(steps(j.line, j.near_end, j.far - j.near));
            }
        }
    }
    for turn in here.iter().filter(|turn| !turn.back) {
        let jog = turn.far - turn.near;
        if jog != 0 && jog.abs() < 2 * RADIUS {
            wanted.push((turn.line, turn.far_end, -jog));
            wanted.push((turn.line, turn.near_end, jog));
        }
    }
    wanted
}

/// The two moves of a step along its side for the port at `end` of a line:
/// first toward `toward`, the line's other stretch, and then away, or the
/// other way round when the other stretch is nearer than a step.
fn steps(line: usize, end: Option<bool>, toward: i32) -> [(usize, Option<bool>, i32); 2] {
    let way = if toward.abs() > STEP {
        toward.signum()
    } else {
        -toward.signum()
    };
    [(line, end, way * STEP), (line, end, -way * STEP)]
}

/// Whether the port at one end of a line, its start or its end, can move
/// `by` along its side: it stays on the side, and clear of the side's
/// other ports.
pub fn fits<'k>(
    ports: &Ports,
    nudges: &BTreeMap<(usize, bool), i32>,
    end_of: &impl Fn(usize, bool) -> (&'k str, Side, i32),
    line: usize,
    start: bool,
    by: i32,
) -> bool {
    let (key, side, length) = end_of(line, start);
    let at = |line: usize, start: bool| {
        ports.offset(key, side, line, length) + nudges.get(&(line, start)).copied().unwrap_or(0)
    };
    let moved = at(line, start) + by;
    moved.abs() <= length / 2 - STEP
        && ports
            .lines(key, side)
            .into_iter()
            .flat_map(|other| [(other, true), (other, false)])
            .filter(|&(other, end)| {
                (other, end) != (line, start) && (end_of(other, end).0, end_of(other, end).1) == (key, side)
            })
            .all(|(other, end)| (moved - at(other, end)).abs() >= STEP)
}
