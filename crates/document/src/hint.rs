//! Where a file's own choice draws worse than another it could make. A
//! hint never stops a diagram from being drawn: the file says where its
//! elements go, and it is drawn as it says. A hint says what to write
//! instead, for whoever writes the file.

use diagram_format::{Body, Diagram, Span};

use crate::geometry::meetings;
use crate::layout;
use crate::scene::{Point, Shape};

/// One hint: what is drawn worse, where the file chose it, and what to
/// write instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub code: &'static str,
    pub message: String,
    pub help: String,
    pub span: Span,
}

/// The most boxes of one tier or rank whose every order is tried.
const ORDERS_UP_TO: usize = 5;

/// What a diagram's file chose that draws worse than another choice would.
pub fn hints(diagram: &Diagram) -> Vec<Hint> {
    match &diagram.body {
        Body::Architecture(body) => tier_orders(diagram, body),
        Body::Dependency(body) => rank_orders(diagram, body),
        _ => Vec::new(),
    }
}

/// A tier whose boxes, in another order, have the lines meet fewer times.
/// Each tier is tried on its own, with every other tier as written.
fn tier_orders(diagram: &Diagram, body: &diagram_format::Architecture) -> Vec<Hint> {
    let written = crossings(diagram);
    if written == 0 {
        return Vec::new();
    }
    let mut hints = Vec::new();
    for (index, tier) in body.tiers.iter().enumerate() {
        if tier.keys.len() < 2 || tier.keys.len() > ORDERS_UP_TO {
            continue;
        }
        let best = orders(&tier.keys)
            .into_iter()
            .filter(|order| *order != tier.keys)
            .map(|order| {
                let mut tried = diagram.clone();
                if let Body::Architecture(body) = &mut tried.body {
                    body.tiers[index].keys = order.clone();
                }
                (crossings(&tried), order)
            })
            .min_by_key(|(crossed, _)| *crossed);
        if let Some((crossed, order)) = best.filter(|(crossed, _)| *crossed < written) {
            hints.push(Hint {
                code: "tier-order-crosses",
                message: format!("in this order, lines meet {}", times(written)),
                help: format!("write `tier {}`, and they meet {}", order.join(" "), times(crossed)),
                span: tier.span,
            });
        }
    }
    hints
}

/// A rank whose boxes, written in another order, have the lines cross
/// fewer times. A rank is the boxes the same number of dependencies along,
/// drawn in the order the file writes them; each rank is tried on its own,
/// its boxes moved among the places the file gives them, every other box
/// where it is written. The hint is at the first box of the rank.
fn rank_orders(diagram: &Diagram, body: &diagram_format::Dependency) -> Vec<Hint> {
    let Ok(ranks) = diagram_format::rules::ranks(body) else {
        return Vec::new();
    };
    let written = crossings(diagram);
    if written == 0 {
        return Vec::new();
    }
    let count = ranks.values().max().map_or(0, |rank| rank + 1);
    let mut hints = Vec::new();
    for rank in 0..count {
        // Where in the file the rank's boxes are written, and their keys.
        let places: Vec<usize> = (0..body.boxes.len())
            .filter(|&n| ranks.get(&body.boxes[n].key) == Some(&rank))
            .collect();
        if places.len() < 2 || places.len() > ORDERS_UP_TO {
            continue;
        }
        let keys: Vec<String> = places.iter().map(|&n| body.boxes[n].key.clone()).collect();
        let best = orders(&keys)
            .into_iter()
            .filter(|order| *order != keys)
            .map(|order| {
                let mut tried = diagram.clone();
                if let Body::Dependency(tried) = &mut tried.body {
                    for (&place, key) in places.iter().zip(&order) {
                        tried.boxes[place] = body.boxes.iter().find(|item| item.key == *key).unwrap().clone();
                    }
                }
                (crossings(&tried), order)
            })
            .min_by_key(|(crossed, _)| *crossed);
        if let Some((crossed, order)) = best.filter(|(crossed, _)| *crossed < written) {
            hints.push(Hint {
                code: "rank-order-crosses",
                message: format!(
                    "the boxes of this rank, {}, make lines meet {}",
                    keys.join(", "),
                    times(written)
                ),
                help: format!(
                    "write them in the order {}, and they meet {}",
                    order.join(", "),
                    times(crossed)
                ),
                span: body.boxes[places[0]].span,
            });
        }
    }
    hints
}

fn times(n: usize) -> String {
    match n {
        0 => "nowhere".to_string(),
        1 => "once".to_string(),
        2 => "twice".to_string(),
        n => format!("{n} times"),
    }
}

/// Every order of `keys`.
fn orders(keys: &[String]) -> Vec<Vec<String>> {
    if keys.len() <= 1 {
        return vec![keys.to_vec()];
    }
    let mut all = Vec::new();
    for (index, key) in keys.iter().enumerate() {
        let mut rest = keys.to_vec();
        rest.remove(index);
        for mut order in orders(&rest) {
            order.insert(0, key.clone());
            all.push(order);
        }
    }
    all
}

/// How many times the lines of a diagram meet, as it is laid out: each
/// point where two cross or touch, and each stretch two run along together.
fn crossings(diagram: &Diagram) -> usize {
    let scene = layout(diagram);
    let lines: Vec<&[Point]> = scene
        .elements
        .iter()
        .filter_map(|element| match &element.shape {
            Shape::Line(line) => Some(line.points.as_slice()),
            _ => None,
        })
        .collect();
    let mut count = 0;
    for (k, a) in lines.iter().enumerate() {
        for b in &lines[k + 1..] {
            count += meetings(a, b);
        }
    }
    count
}
