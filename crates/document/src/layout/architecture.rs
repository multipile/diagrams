//! The parts of a system, and what talks to what. Boxes sit in tiers along
//! the flow: a tier is a column when the flow runs right, and a row when it
//! runs down. A box may hold boxes, drawn inside it. A zone is drawn around
//! its tier, dashed when it is a trust boundary.

use std::collections::BTreeMap;

use diagram_format::{ArchBox, Architecture, Flow, Item};

use super::{Canvas, item_box};
use crate::geometry::{
    CLEARANCE, GRID, Ports, RADIUS, Side, across, box_height, box_width, label_width, meetings, simplify, snap,
};
use crate::scene::*;

const GAP_TIER: i32 = 112;
const GAP_BOX: i32 = 32;
const HOLD_PAD: i32 = 12;
const HOLD_HEAD: i32 = 40;
const ZONE_PAD: i32 = 16;
const ZONE_LABEL: i32 = 24;
/// How far past a tier a line first turns, and how far apart the turns in
/// one channel are.
const TURN: i32 = 28;
const PITCH: i32 = 8;
/// How many times the ports may move before the routes are kept.
const ROUNDS: usize = 32;
/// How far a port moves along its side to keep its stretch out of line with
/// another's, and how far it stays from the side's ends and other ports.
const STEP: i32 = 12;

pub fn layout(body: &Architecture, canvas: &mut Canvas) {
    let down = body.flow == Flow::Down;
    let boxes: BTreeMap<&str, &ArchBox> = body.boxes.iter().map(|held| (held.item.key.as_str(), held)).collect();
    let held_items: Vec<&Item> = body.boxes.iter().flat_map(|held| &held.holds).collect();
    let hw = held_items
        .iter()
        .map(|item| box_width(&item.name, item.sub.as_deref(), None))
        .max()
        .unwrap_or(0);
    let hh = box_height(held_items.iter().any(|item| item.sub.is_some()), false);
    let size = |held: &ArchBox| -> (i32, i32) {
        let own = box_width(&held.item.name, held.item.sub.as_deref(), None);
        let n = held.holds.len() as i32;
        if n == 0 {
            return (own, box_height(held.item.sub.is_some(), false));
        }
        if down {
            (
                own.max(n * hw + (n - 1) * HOLD_PAD + 2 * HOLD_PAD),
                HOLD_HEAD + hh + HOLD_PAD,
            )
        } else {
            (
                own.max(hw + 2 * HOLD_PAD),
                HOLD_HEAD + n * hh + (n - 1) * HOLD_PAD + HOLD_PAD,
            )
        }
    };

    // Each tier's extent along the flow, and across it.
    let extents: Vec<(i32, i32)> = body
        .tiers
        .iter()
        .map(|tier| {
            let sizes: Vec<(i32, i32)> = tier.keys.iter().map(|key| size(boxes[key.as_str()])).collect();
            let n = sizes.len() as i32;
            if down {
                let along = sizes.iter().map(|s| s.1).max().unwrap_or(0);
                let across = sizes.iter().map(|s| s.0).sum::<i32>() + (n - 1) * GAP_BOX;
                (along, across)
            } else {
                let along = sizes.iter().map(|s| s.0).max().unwrap_or(0);
                let across = sizes.iter().map(|s| s.1).sum::<i32>() + (n - 1) * GAP_BOX;
                (along, across)
            }
        })
        .collect();
    let widest = extents.iter().map(|e| e.1).max().unwrap_or(0);

    let mut rects: BTreeMap<&str, Rect> = BTreeMap::new();
    let mut tier_of: BTreeMap<&str, usize> = BTreeMap::new();
    let mut tier_rects: Vec<Rect> = Vec::new();
    // The gap between tiers is wide enough for the widest label, which sits
    // on the stretch of a line that crosses it.
    let gap = if down {
        GAP_TIER
    } else {
        body.lines
            .iter()
            .filter_map(|line| line.label.as_deref())
            .map(|label| snap(label_width(&label.to_uppercase()) + 72))
            .max()
            .unwrap_or(0)
            .max(GAP_TIER)
    };
    let mut at = 0;
    for (index, tier) in body.tiers.iter().enumerate() {
        let (along, across) = extents[index];
        let mut cursor = snap((widest - across) / 2);
        if tier.zone.is_some() && index > 0 {
            at += ZONE_PAD;
        }
        let mut tier_rect: Option<Rect> = None;
        for key in &tier.keys {
            let held = boxes[key.as_str()];
            let (w, h) = size(held);
            let rect = if down {
                Rect::new(cursor, at + (along - h) / 2, w, h)
            } else {
                Rect::new(at + (along - w) / 2, cursor, w, h)
            };
            cursor += if down { w } else { h } + GAP_BOX;
            rects.insert(key.as_str(), rect);
            tier_of.insert(key.as_str(), index);
            tier_rect = Some(tier_rect.map_or(rect, |r| r.union(&rect)));
            for (n, inner) in held.holds.iter().enumerate() {
                let n = n as i32;
                let inner_rect = if down {
                    Rect::new(rect.x + HOLD_PAD + n * (hw + HOLD_PAD), rect.y + HOLD_HEAD, hw, hh)
                } else {
                    Rect::new(
                        rect.x + (rect.w - hw) / 2,
                        rect.y + HOLD_HEAD + n * (hh + HOLD_PAD),
                        hw,
                        hh,
                    )
                };
                rects.insert(inner.key.as_str(), inner_rect);
                tier_of.insert(inner.key.as_str(), index);
            }
        }
        tier_rects.push(tier_rect.unwrap_or_default());
        at += along + gap;
    }

    for (index, tier) in body.tiers.iter().enumerate() {
        if let Some(zone) = &tier.zone {
            let inner = tier_rects[index].grow(ZONE_PAD);
            let rect = Rect::new(inner.x, inner.y - ZONE_LABEL, inner.w, inner.h + ZONE_LABEL);
            canvas.push(
                format!("zone:{index}"),
                2 * index as u32,
                Shape::Region(RegionShape {
                    pile: None,
                    rect,
                    kind: RegionKind::Zone,
                    label: Some(zone.label.clone()),
                    sub: None,
                    dashed: zone.trust,
                    focal: false,
                }),
            );
        }
    }

    // Which side of each box each line leaves and enters by.
    let sides: Vec<(Side, Side)> = body
        .lines
        .iter()
        .map(|line| {
            let (from, to) = (tier_of[line.from.as_str()], tier_of[line.to.as_str()]);
            match (from.cmp(&to), down) {
                (std::cmp::Ordering::Less, false) => (Side::Right, Side::Left),
                (std::cmp::Ordering::Greater, false) => (Side::Left, Side::Right),
                (std::cmp::Ordering::Equal, false) => (Side::Right, Side::Right),
                (std::cmp::Ordering::Less, true) => (Side::Bottom, Side::Top),
                (std::cmp::Ordering::Greater, true) => (Side::Top, Side::Bottom),
                (std::cmp::Ordering::Equal, true) => (Side::Bottom, Side::Bottom),
            }
        })
        .collect();
    let mut ports = Ports::default();
    for (index, (line, (out, into))) in body.lines.iter().zip(&sides).enumerate() {
        let (from, to) = (rects[line.from.as_str()], rects[line.to.as_str()]);
        let along = |rect: &Rect, side: Side| if side.vertical() { rect.cx() } else { rect.cy() };
        ports.add(&line.from, *out, index, along(&to, *out));
        ports.add(&line.to, *into, index, along(&from, *into));
    }
    // The channel after each tier: from its far edge along the flow to the
    // next tier's near edge, or open beyond the last tier. A line turns in a
    // channel at a distance from the tier, never from its own box, so that
    // no turn runs beside a wider box of the same tier.
    let along = |p: Point| if down { p.y } else { p.x };
    let side = |p: Point| if down { p.x } else { p.y };
    let channels: Vec<(i32, Option<i32>)> = tier_rects
        .iter()
        .enumerate()
        .map(|(index, rect)| {
            let far = if down { rect.bottom() } else { rect.right() };
            let next = tier_rects.get(index + 1).map(|r| if down { r.y } else { r.x });
            (far, next)
        })
        .collect();
    let first = |channel: usize| channels[channel].0 + TURN;

    // Where each end of each line meets its box, along the side: its port,
    // moved by `nudges` when a stretch would otherwise line up with
    // another line's in the same channel.
    let length = |rect: &Rect, side: Side| if side.vertical() { rect.w } else { rect.h };
    let end_of = |index: usize, start: bool| {
        let (line, (out, into)) = (&body.lines[index], sides[index]);
        if start {
            (line.from.as_str(), out, length(&rects[line.from.as_str()], out))
        } else {
            (line.to.as_str(), into, length(&rects[line.to.as_str()], into))
        }
    };
    let mut nudges: BTreeMap<(usize, bool), i32> = BTreeMap::new();
    let offset = |nudges: &BTreeMap<(usize, bool), i32>, index: usize, start: bool| {
        let (key, side, length) = end_of(index, start);
        ports.offset(key, side, index, length) + nudges.get(&(index, start)).copied().unwrap_or(0)
    };
    // The boxes each line passes: all but its ends, the boxes that hold
    // them, and the boxes they hold.
    let passed: Vec<Vec<Rect>> = body
        .lines
        .iter()
        .map(|line| {
            let mut met: Vec<&str> = vec![line.from.as_str(), line.to.as_str()];
            for held in &body.boxes {
                let key = held.item.key.as_str();
                let inner: Vec<&str> = held.holds.iter().map(|inner| inner.key.as_str()).collect();
                if inner.iter().any(|inner| met[..2].contains(inner)) {
                    met.push(key);
                }
                if met[..2].contains(&key) {
                    met.extend(inner);
                }
            }
            rects
                .iter()
                .filter(|(key, _)| !met.contains(key))
                .map(|(_, rect)| *rect)
                .collect()
        })
        .collect();
    let mut round = 0;
    let routes = loop {
        // Each line's route, turning at the first place in each channel it
        // crosses, and every turn noted by channel.
        let mut routes: Vec<Vec<Point>> = Vec::new();
        let mut turns: BTreeMap<usize, Vec<Turn>> = BTreeMap::new();
        for (index, (line, (out, into))) in body.lines.iter().zip(&sides).enumerate() {
            let (from, to) = (rects[line.from.as_str()], rects[line.to.as_str()]);
            let start = out.point(&from, offset(&nudges, index, true));
            let end = into.point(&to, offset(&nudges, index, false));
            let others = &passed[index];
            let (a, b) = (tier_of[line.from.as_str()], tier_of[line.to.as_str()]);
            let route = if a == b {
                // Within one tier: out to the side, along, and back in.
                let at = first(a);
                turns.entry(a).or_default().push(Turn {
                    line: index,
                    segment: 1,
                    near: side(start),
                    far: side(end),
                    near_end: None,
                    far_end: None,
                    back: true,
                });
                if down {
                    vec![start, Point::new(start.x, at), Point::new(end.x, at), end]
                } else {
                    vec![start, Point::new(at, start.y), Point::new(at, end.y), end]
                }
            } else {
                let (leaving, arriving) = if a < b { (a, b - 1) } else { (a - 1, b) };
                let (leave, arrive) = (first(leaving), first(arriving));
                let route = simplify(across(start, end, down, leave, arrive, others, CLEARANCE));
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
                    // The stretch before the turn ends at the line's start
                    // when it is the first, and the one after at its end
                    // when it is the last.
                    let before = (segment == 1).then_some(true);
                    let after = (segment + 1 == last).then_some(false);
                    let turn = if a < b {
                        (side(p), side(q), before, after)
                    } else {
                        (side(q), side(p), after, before)
                    };
                    turns.entry(channel).or_default().push(Turn {
                        line: index,
                        segment,
                        near: turn.0,
                        far: turn.1,
                        near_end: turn.2,
                        far_end: turn.3,
                        back: false,
                    });
                }
                route
            };
            routes.push(route);
        }

        // In each channel, every turn at its own place, in the order that
        // crosses the fewest lines. When a stretch still lines up with
        // another's, running into it or on from where it stops, the port at
        // the end of one of them moves along its box's side, and the routes
        // are made again.
        let mut moved = false;
        for (channel, here) in &turns {
            let (far, next) = channels[*channel];
            let n = here.len() as i32;
            let pitch = match next {
                Some(next) if n > 1 => ((next - far - 2 * TURN) / (n - 1)).div_euclid(GRID) * GRID,
                _ => PITCH,
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
            // First a stretch that lines up with another's: one of their
            // ends moves a step along its side.
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
            // Then a turn too short to round: the port it leads to moves
            // in line with the stretch before it, and the line runs straight.
            for turn in here.iter().filter(|turn| !turn.back) {
                let jog = turn.far - turn.near;
                if jog != 0 && jog.abs() < 2 * RADIUS {
                    wanted.push((turn.line, turn.far_end, -jog));
                    wanted.push((turn.line, turn.near_end, jog));
                }
            }
            if let Some((end, by)) = wanted.into_iter().find_map(|(line, end, by)| {
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

    for ((line, route), others) in body.lines.iter().zip(routes).zip(&passed) {
        let tier = tier_of[line.from.as_str()].max(tier_of[line.to.as_str()]);
        canvas.line_past(
            format!("line:{}->{}", line.from, line.to),
            2 * tier as u32 + 1,
            simplify(route),
            line.kind,
            line.focal,
            line.label.as_deref(),
            others,
        );
    }

    for held in &body.boxes {
        let step = 2 * tier_of[held.item.key.as_str()] as u32;
        let mut shape = item_box(&held.item, rects[held.item.key.as_str()]);
        shape.top = !held.holds.is_empty();
        canvas.push(format!("box:{}", held.item.key), step, Shape::Box(shape));
        for inner in &held.holds {
            canvas.item(format!("box:{}", inner.key), step, inner, rects[inner.key.as_str()]);
        }
    }
}

/// Where a line turns in a channel: the segment of its route that crosses
/// the channel, from the side `near` the tier before it to the side `far`
/// toward the tier after, across the flow. A line that comes `back` to the
/// tier it left has both its ends on the near side.
struct Turn {
    line: usize,
    segment: usize,
    near: i32,
    far: i32,
    /// Which end of the line, its start or its end, the stretch on each
    /// side of the turn reaches, if it reaches one.
    near_end: Option<bool>,
    far_end: Option<bool>,
    back: bool,
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
fn channel_order(turns: &[Turn]) -> Vec<usize> {
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
fn fits<'k>(
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
