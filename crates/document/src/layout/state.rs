//! States, and the events that move between them. States are laid out along
//! the flow, in the order the transitions reach them from the start. A
//! transition to the next state is a straight line; one further on arcs
//! over the states between, and one back runs under them. A transition from
//! a state to itself loops over it. A filled dot leads into the start, and
//! each end leads to a ringed dot.

use std::collections::{BTreeMap, VecDeque};

use diagram_format::{Flow, LineKind, StateMachine};

use super::{Canvas, line_shape, uniform_height, uniform_width};
use crate::geometry::{Side, label_width, mono_width, snap};
use crate::scene::*;

const GAP_MIN: i32 = 96;
const TRACK: i32 = 28;
const TRACK_STEP: i32 = 22;
const DOT: i32 = 40;

pub fn layout(body: &StateMachine, canvas: &mut Canvas) {
    let order = order(body);
    let index: BTreeMap<&str, usize> = order.iter().enumerate().map(|(i, key)| (*key, i)).collect();
    let items: Vec<_> = body.states.iter().map(|state| &state.item).collect();
    let w = uniform_width(items.iter().copied());
    let h = uniform_height(items.iter().copied());
    let right = body.flow == Flow::Right;

    // The gap after each state is wide enough for the labels of the
    // transitions drawn straight across it.
    let mut gaps = vec![GAP_MIN; order.len()];
    for transition in &body.transitions {
        let (a, b) = (index[transition.from.as_str()], index[transition.to.as_str()]);
        if b == a + 1 {
            let need = if right {
                label_width(&transition.label()) + 24
            } else {
                40
            };
            gaps[a] = gaps[a].max(snap(need));
        }
    }
    let mut rects: BTreeMap<&str, Rect> = BTreeMap::new();
    let mut at = DOT + 16;
    for key in &order {
        let rect = if right {
            Rect::new(at, 0, w, h)
        } else {
            Rect::new(0, at, w, h)
        };
        rects.insert(key, rect);
        at += if right { w } else { h } + gaps[index[key]];
    }

    let (out, into, upper, lower) = if right {
        (Side::Right, Side::Left, Side::Top, Side::Bottom)
    } else {
        (Side::Bottom, Side::Top, Side::Left, Side::Right)
    };
    let edge = |side: Side| -> i32 {
        let any = rects.values().next().copied().unwrap_or_default();
        match side {
            Side::Top => any.y,
            Side::Bottom => any.bottom(),
            Side::Left => any.x,
            Side::Right => any.right(),
        }
    };
    let around = |start: Point, end: Point, side: Side, track: i32| -> Vec<Point> {
        if side.vertical() {
            vec![start, Point::new(start.x, track), Point::new(end.x, track), end]
        } else {
            vec![start, Point::new(track, start.y), Point::new(track, end.y), end]
        }
    };

    // The start's dot, and its line into the start.
    if let Some(start) = body.states.iter().find(|state| state.start) {
        let rect = rects[start.item.key.as_str()];
        let center = if right {
            Point::new(rect.x - DOT, rect.cy())
        } else {
            Point::new(rect.cx(), rect.y - DOT)
        };
        canvas.push("start", 0, Shape::Dot(DotShape { center, ring: false }));
        let from = if right {
            Point::new(center.x + 6, center.y)
        } else {
            Point::new(center.x, center.y + 6)
        };
        canvas.push(
            "line:start",
            0,
            Shape::Line(line_shape(
                vec![from, into.point(&rect, 0)],
                Head::Arrow,
                false,
                Tone::Muted,
                None,
            )),
        );
    }

    // Every transition that is not drawn straight goes around: over the
    // states or under them. One forward goes over and one back goes under,
    // unless that side already holds an arc it would cross; shorter arcs
    // are placed first, so they take the inner tracks.
    let mut arcs: Vec<(usize, Side, i32)> = Vec::new();
    let mut placed: Vec<(Side, usize, usize, i32)> = Vec::new();
    let mut around_order: Vec<usize> = (0..body.transitions.len())
        .filter(|&n| {
            let t = &body.transitions[n];
            let (a, b) = (index[t.from.as_str()], index[t.to.as_str()]);
            b != a + 1
        })
        .collect();
    around_order.sort_by_key(|&n| {
        let t = &body.transitions[n];
        (index[t.from.as_str()].abs_diff(index[t.to.as_str()]), n)
    });
    for n in around_order {
        let t = &body.transitions[n];
        let (a, b) = (index[t.from.as_str()], index[t.to.as_str()]);
        let (lo, hi) = (a.min(b), a.max(b));
        let crosses = |side: Side| {
            placed
                .iter()
                .any(|(s, l, h, _)| *s == side && ((*l < lo && lo < *h && *h < hi) || (lo < *l && *l < hi && hi < *h)))
        };
        let preferred = if b >= a { upper } else { lower };
        let other = if preferred == upper { lower } else { upper };
        let side = if a != b && crosses(preferred) && !crosses(other) {
            other
        } else {
            preferred
        };
        let track = placed
            .iter()
            .filter(|(s, l, h, _)| *s == side && *l <= hi && lo <= *h)
            .map(|(_, _, _, track)| track + 1)
            .max()
            .unwrap_or(0);
        placed.push((side, lo, hi, track));
        arcs.push((n, side, track));
    }
    let lower_tracks = placed
        .iter()
        .filter(|(side, ..)| *side == lower)
        .map(|(.., track)| track + 1)
        .max()
        .unwrap_or(0);

    let mut straight: BTreeMap<(usize, usize), i32> = BTreeMap::new();
    for (n, transition) in body.transitions.iter().enumerate() {
        let (a, b) = (index[transition.from.as_str()], index[transition.to.as_str()]);
        let (from, to) = (rects[transition.from.as_str()], rects[transition.to.as_str()]);
        let points = if let Some((_, side, track)) = arcs.iter().find(|(m, ..)| *m == n) {
            let distance = TRACK + track * TRACK_STEP;
            let coordinate = if *side == upper {
                edge(*side) - distance
            } else {
                edge(*side) + distance
            };
            if a == b {
                around(side.point(&from, -16), side.point(&from, 16), *side, coordinate)
            } else {
                let toward = if b > a { 8 } else { -8 };
                around(side.point(&from, toward), side.point(&to, -toward), *side, coordinate)
            }
        } else {
            let k = straight.entry((a, b)).or_default();
            let offset = *k * 12 - 6 * i32::from(*k > 0);
            *k += 1;
            vec![out.point(&from, offset), into.point(&to, offset)]
        };
        let label = transition.label();
        canvas.line(
            format!("transition:{}->{}:{}", transition.from, transition.to, transition.on),
            2 * a.max(b) as u32 + 1,
            points,
            LineKind::Call,
            transition.focal,
            Some(&label),
        );
    }

    let lowest = edge(lower) + TRACK + lower_tracks * TRACK_STEP;
    for state in &body.states {
        let rect = rects[state.item.key.as_str()];
        let step = 2 * index[state.item.key.as_str()] as u32;
        if state.end {
            // Past the state along the flow when it is the last; otherwise
            // beyond the lines that run under the states.
            let last = index[state.item.key.as_str()] + 1 == order.len();
            let (from, center) = if last {
                let from = out.point(&rect, 0);
                let center = if right {
                    Point::new(rect.right() + DOT, rect.cy())
                } else {
                    Point::new(rect.cx(), rect.bottom() + DOT)
                };
                (from, center)
            } else {
                let from = lower.point(&rect, 24);
                let beyond = lowest + 8;
                let center = if right {
                    Point::new(from.x, beyond + 8)
                } else {
                    Point::new(beyond + 8, from.y)
                };
                (from, center)
            };
            // The line meets the ring from beside it when the ring is past
            // the last state across a flow to the right, or beyond the lines
            // under a flow downward; otherwise from above.
            let to = if right == last {
                Point::new(center.x - 8, center.y)
            } else {
                Point::new(center.x, center.y - 8)
            };
            canvas.push(
                format!("line:end:{}", state.item.key),
                step + 1,
                Shape::Line(line_shape(vec![from, to], Head::Arrow, false, Tone::Muted, None)),
            );
            canvas.push(
                format!("end:{}", state.item.key),
                step + 1,
                Shape::Dot(DotShape { center, ring: true }),
            );
        }
        canvas.item(format!("state:{}", state.item.key), step, &state.item, rect);
    }

    if let Some(any) = &body.from_any {
        let name = body
            .states
            .iter()
            .find(|state| state.item.key == any.to)
            .map_or(any.to.as_str(), |state| state.item.name.as_str());
        let text = format!("FROM ANY STATE → {} ON {}", name.to_uppercase(), any.on.to_uppercase());
        let bottom = rects
            .values()
            .map(Rect::bottom)
            .max()
            .unwrap_or(0)
            .max(if right { lowest } else { 0 });
        canvas.push(
            "any",
            2 * order.len() as u32,
            Shape::Text(TextShape {
                at: Point::new(0, bottom + 32),
                width: mono_width(&text),
                text,
                style: TextStyle::Eyebrow,
                focal: false,
            }),
        );
    }
}

/// The states in the order the transitions reach them from the start, and
/// then those they never reach, in the order written.
fn order(body: &StateMachine) -> Vec<&str> {
    let mut order: Vec<&str> = Vec::new();
    let mut queue: VecDeque<&str> = body
        .states
        .iter()
        .filter(|state| state.start)
        .map(|state| state.item.key.as_str())
        .collect();
    while let Some(key) = queue.pop_front() {
        if order.contains(&key) {
            continue;
        }
        order.push(key);
        for transition in body.transitions.iter().filter(|t| t.from == key) {
            if !order.contains(&transition.to.as_str()) {
                queue.push_back(transition.to.as_str());
            }
        }
        if let Some(any) = &body.from_any
            && !order.contains(&any.to.as_str())
            && queue.is_empty()
        {
            queue.push_back(any.to.as_str());
        }
    }
    for state in &body.states {
        if !order.contains(&state.item.key.as_str()) {
            order.push(state.item.key.as_str());
        }
    }
    order
}
