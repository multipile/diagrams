//! Messages between actors, in time order. Actors stand in a row at the
//! top, in the order written, each with a lifeline below. Time runs down,
//! one row a message. Where an actor holds control between a call and its
//! reply, its lifeline is drawn wider. An opt, loop or alt is a frame
//! around its messages, with its guard at the top left.

use std::collections::BTreeMap;

use diagram_format::{Arrow, ArrowKind, FragmentKind, Message, Sequence};

use super::{Canvas, line_shape, uniform_height, uniform_width};
use crate::geometry::{label_width, mono_width, snap};
use crate::scene::*;

const ROW: i32 = 40;
const SELF_W: i32 = 40;
const BAR: i32 = 8;

pub fn layout(body: &Sequence, canvas: &mut Canvas) {
    let w = uniform_width(&body.actors);
    let h = uniform_height(&body.actors);
    let index: BTreeMap<&str, usize> = body
        .actors
        .iter()
        .enumerate()
        .map(|(i, actor)| (actor.key.as_str(), i))
        .collect();

    // Rows: each arrow in time order, with the y it is drawn at, and each
    // fragment with the rows it spans.
    let mut rows: Vec<(&Arrow, i32)> = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut y = h + 12;
    walk(&body.messages, &mut y, &mut rows, &mut frames);
    let bottom = y + 24;

    // The space between neighbouring actors fits the labels between them.
    let mut pitch = vec![w + 48; body.actors.len()];
    for (arrow, _) in &rows {
        let (a, b) = (index[arrow.from.as_str()], index[arrow.to.as_str()]);
        let need = if a == b {
            SELF_W + label_width(&arrow.label) + 24
        } else {
            snap((label_width(&arrow.label) + 32) / (a.abs_diff(b) as i32))
        };
        let gap = a.min(b);
        pitch[gap] = pitch[gap].max(need.max(w + 48));
    }
    let mut centers = Vec::new();
    let mut x = w / 2;
    for (i, _) in body.actors.iter().enumerate() {
        centers.push(x);
        x += pitch[i];
    }
    let left = centers[0] - w / 2;
    let right = centers.last().copied().unwrap_or(0) + w / 2 + SELF_W;

    // Where each actor holds control: from a call to it, to its reply.
    let mut bars: Vec<(usize, i32, i32)> = Vec::new();
    for (n, (arrow, y1)) in rows.iter().enumerate() {
        if arrow.kind != ArrowKind::Call || arrow.from == arrow.to {
            continue;
        }
        if let Some((_, y2)) = rows[n + 1..]
            .iter()
            .find(|(reply, _)| reply.kind == ArrowKind::Reply && reply.from == arrow.to && reply.to == arrow.from)
        {
            bars.push((index[arrow.to.as_str()], *y1, *y2));
        }
    }
    let active = |actor: usize, y: i32| bars.iter().any(|(a, y1, y2)| *a == actor && *y1 <= y && y <= *y2);

    for (i, actor) in body.actors.iter().enumerate() {
        canvas.item(
            format!("actor:{}", actor.key),
            0,
            actor,
            Rect::new(centers[i] - w / 2, 0, w, h),
        );
        canvas.push(
            format!("lifeline:{}", actor.key),
            0,
            Shape::Region(RegionShape {
                pile: None,
                rect: Rect::new(centers[i], h, 0, bottom - h),
                kind: RegionKind::Lifeline,
                label: None,
                sub: None,
                dashed: true,
                focal: false,
            }),
        );
    }

    let step_of = |y: i32| -> u32 { rows.iter().position(|(_, at)| *at >= y).unwrap_or(rows.len()) as u32 + 1 };
    for (n, frame) in frames.iter().enumerate() {
        let rect = Rect::new(left - 16, frame.top, right - left + 32, frame.bottom - frame.top);
        canvas.push(
            format!("fragment:{n}"),
            step_of(frame.top),
            Shape::Region(RegionShape {
                pile: None,
                rect,
                kind: RegionKind::Fragment,
                label: Some(format!("{} [{}]", frame.kind, frame.guards[0])),
                sub: None,
                dashed: false,
                focal: false,
            }),
        );
        if let Some(divider) = frame.divider {
            canvas.push(
                format!("fragment:{n}:else"),
                step_of(divider),
                Shape::Line(line_shape(
                    vec![Point::new(rect.x, divider), Point::new(rect.right(), divider)],
                    Head::None,
                    true,
                    Tone::Muted,
                    None,
                )),
            );
            let text = format!("[{}]", frame.guards[1]).to_uppercase();
            canvas.push(
                format!("fragment:{n}:guard"),
                step_of(divider),
                Shape::Text(TextShape {
                    at: Point::new(rect.x + 8, divider + 4),
                    width: mono_width(&text),
                    text,
                    style: TextStyle::Eyebrow,
                    focal: false,
                }),
            );
        }
    }

    for (n, (actor, y1, y2)) in bars.iter().enumerate() {
        canvas.push(
            format!("activation:{}:{n}", body.actors[*actor].key),
            step_of(*y1),
            Shape::Region(RegionShape {
                pile: None,
                rect: Rect::new(centers[*actor] - BAR / 2, *y1 - 4, BAR, *y2 - *y1 + 8),
                kind: RegionKind::Activation,
                label: None,
                sub: None,
                dashed: false,
                focal: false,
            }),
        );
    }

    for (n, (arrow, y)) in rows.iter().enumerate() {
        let (a, b) = (index[arrow.from.as_str()], index[arrow.to.as_str()]);
        let (head, dashed) = match arrow.kind {
            ArrowKind::Call | ArrowKind::SelfCall => (Head::Arrow, false),
            ArrowKind::Reply => (Head::Arrow, true),
            ArrowKind::Notify => (Head::Open, true),
        };
        let tone = if arrow.focal {
            Tone::Accent
        } else if arrow.link {
            Tone::Link
        } else {
            Tone::Ink
        };
        let label = arrow.label.to_uppercase();
        let (points, at) = if a == b {
            let x = centers[a] + if active(a, *y) { BAR / 2 } else { 0 };
            let points = vec![
                Point::new(x, *y - 8),
                Point::new(x + SELF_W, *y - 8),
                Point::new(x + SELF_W, *y + 8),
                Point::new(x, *y + 8),
            ];
            let at = Point::new(x + SELF_W + 8 + label_width(&label) / 2, *y);
            (points, at)
        } else {
            let sign = if b > a { 1 } else { -1 };
            let start = centers[a] + if active(a, *y) { sign * BAR / 2 } else { 0 };
            let end = centers[b] - if active(b, *y) { sign * BAR / 2 } else { 0 };
            let at = Point::new((start + end) / 2, *y - 10);
            (vec![Point::new(start, *y), Point::new(end, *y)], at)
        };
        let mut shape = line_shape(points, head, dashed, tone, None);
        shape.label = Some(Label { text: label, at });
        canvas.push(
            format!("message:{}->{}:{}", arrow.from, arrow.to, arrow.label),
            n as u32 + 1,
            Shape::Line(shape),
        );
    }
}

struct Frame {
    kind: &'static str,
    guards: Vec<String>,
    top: i32,
    bottom: i32,
    divider: Option<i32>,
}

fn walk<'m>(messages: &'m [Message], y: &mut i32, rows: &mut Vec<(&'m Arrow, i32)>, frames: &mut Vec<Frame>) {
    for message in messages {
        match message {
            Message::Arrow(arrow) => {
                *y += ROW;
                rows.push((arrow, *y));
                if arrow.kind == ArrowKind::SelfCall {
                    *y += 8;
                }
            }
            Message::Fragment(fragment) => {
                let top = *y + 16;
                *y = top + 16;
                let mut divider = None;
                for (n, case) in fragment.cases.iter().enumerate() {
                    if n > 0 {
                        *y += 16;
                        divider = Some(*y);
                        *y += 16;
                    }
                    walk(&case.messages, y, rows, frames);
                }
                *y += 16;
                frames.push(Frame {
                    kind: match fragment.kind {
                        FragmentKind::Opt => "OPT",
                        FragmentKind::Loop => "LOOP",
                        FragmentKind::Alt => "ALT",
                    },
                    guards: fragment.cases.iter().map(|case| case.guard.clone()).collect(),
                    top,
                    bottom: *y,
                    divider,
                });
            }
        }
    }
}
