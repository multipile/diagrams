//! The parts of a system, and what talks to what. Boxes sit in tiers along
//! the flow: a tier is a column when the flow runs right, and a row when it
//! runs down. A box may hold boxes, drawn inside it. A zone is drawn around
//! its tier, dashed when it is a trust boundary.

use std::collections::BTreeMap;

use diagram_format::{ArchBox, Architecture, Flow, Item};

use super::{Canvas, item_box};
use crate::geometry::{Ports, Side, across, box_height, box_width, label_width, snap};
use crate::scene::*;

const GAP_TIER: i32 = 112;
const GAP_BOX: i32 = 32;
const HOLD_PAD: i32 = 12;
const HOLD_HEAD: i32 = 40;
const ZONE_PAD: i32 = 16;
const ZONE_LABEL: i32 = 24;

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
    let all: Vec<(&str, Rect)> = rects.iter().map(|(key, rect)| (*key, *rect)).collect();
    let mut channel: BTreeMap<i32, i32> = BTreeMap::new();
    for (index, (line, (out, into))) in body.lines.iter().zip(&sides).enumerate() {
        let (from, to) = (rects[line.from.as_str()], rects[line.to.as_str()]);
        let length = |rect: &Rect, side: Side| if side.vertical() { rect.w } else { rect.h };
        let start = out.point(&from, ports.offset(&line.from, *out, index, length(&from, *out)));
        let end = into.point(&to, ports.offset(&line.to, *into, index, length(&to, *into)));
        let holders: Vec<&str> = body
            .boxes
            .iter()
            .filter(|held| {
                held.holds
                    .iter()
                    .any(|inner| inner.key == line.from || inner.key == line.to)
            })
            .map(|held| held.item.key.as_str())
            .collect();
        let others: Vec<Rect> = all
            .iter()
            .filter(|(key, _)| *key != line.from && *key != line.to && !holders.contains(key))
            .map(|(_, rect)| *rect)
            .collect();
        let points = if out == into {
            // Within one tier: out to the side, along, and back in.
            let tier = tier_rects[tier_of[line.from.as_str()]];
            let slot = channel.entry(-(tier_of[line.from.as_str()] as i32) - 1).or_default();
            let beyond = 20 + *slot * 8;
            *slot += 1;
            if down {
                let y = tier.bottom() + beyond;
                vec![start, Point::new(start.x, y), Point::new(end.x, y), end]
            } else {
                let x = tier.right() + beyond;
                vec![start, Point::new(x, start.y), Point::new(x, end.y), end]
            }
        } else {
            let (leave, arrive) = if down { (start.y, end.y) } else { (start.x, end.x) };
            let step = if arrive > leave { 1 } else { -1 };
            let slot = channel.entry(leave).or_default();
            let offset = (28 + (*slot % 4) * 8) * step;
            *slot += 1;
            across(start, end, down, leave + offset, arrive - step * 28, &others)
        };
        let tier = tier_of[line.from.as_str()].max(tier_of[line.to.as_str()]);
        canvas.line(
            format!("line:{}->{}", line.from, line.to),
            2 * tier as u32 + 1,
            crate::geometry::simplify(points),
            line.kind,
            line.focal,
            line.label.as_deref(),
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
