//! A process run by several teams. Lanes run left to right, one above
//! another, and steps are columns, in order. Each task sits where its lane
//! meets its column, with its lane's badge, its tool, and the data it takes
//! and gives. A line leaves a task on its right, and enters the next from
//! its left in the same lane, or from above or below in another.

use std::collections::BTreeMap;

use diagram_format::{LineKind, Process};

use super::{Canvas, item_box};
use crate::geometry::{box_height, box_width, mono_width, name_width, snap};
use crate::scene::*;

const HEADER_H: i32 = 32;
const LANE_PAD: i32 = 12;
const GAP_COLUMN: i32 = 40;

pub fn layout(body: &Process, canvas: &mut Canvas) {
    let tasks: Vec<_> = body
        .lanes
        .iter()
        .enumerate()
        .flat_map(|(lane, l)| l.tasks.iter().map(move |task| (task, lane)))
        .collect();
    let cell_w = tasks
        .iter()
        .map(|(task, _)| box_width(&task.item.name, task.item.sub.as_deref(), task.tool.as_deref()))
        .chain(body.columns.iter().map(|column| snap(mono_width(&column.label) + 24)))
        .max()
        .unwrap_or(120);
    let h = box_height(
        tasks.iter().any(|(task, _)| task.item.sub.is_some()),
        tasks.iter().any(|(task, _)| task.tool.is_some()),
    );
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
    let column_x: BTreeMap<&str, i32> = body
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| (column.key.as_str(), label_w + index as i32 * (cell_w + GAP_COLUMN)))
        .collect();
    let column_index: BTreeMap<&str, u32> = body
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| (column.key.as_str(), index as u32))
        .collect();
    let total_w = label_w + body.columns.len() as i32 * (cell_w + GAP_COLUMN) - GAP_COLUMN + LANE_PAD;
    let total_h = HEADER_H + body.lanes.len() as i32 * lane_h;

    for (index, column) in body.columns.iter().enumerate() {
        let x = column_x[column.key.as_str()] - GAP_COLUMN / 4;
        canvas.push(
            format!("column:{}", column.key),
            0,
            Shape::Region(RegionShape {
                rect: Rect::new(x, 0, cell_w + GAP_COLUMN / 2, total_h),
                kind: RegionKind::Column,
                label: Some(format!("{:02} {}", index + 1, column.label)),
                sub: None,
                dashed: false,
                focal: column.focal,
            }),
        );
    }
    for (index, lane) in body.lanes.iter().enumerate() {
        canvas.push(
            format!("lane:{}", lane.key),
            0,
            Shape::Region(RegionShape {
                rect: Rect::new(0, HEADER_H + index as i32 * lane_h, total_w, lane_h),
                kind: RegionKind::Lane,
                label: Some(lane.name.clone()),
                sub: Some(lane.badge.clone()),
                dashed: false,
                focal: false,
            }),
        );
    }

    let mut rects: BTreeMap<&str, (Rect, usize)> = BTreeMap::new();
    for (task, lane) in &tasks {
        let rect = Rect::new(
            column_x[task.column.as_str()],
            HEADER_H + *lane as i32 * lane_h + LANE_PAD,
            cell_w,
            h,
        );
        rects.insert(task.item.key.as_str(), (rect, *lane));
    }

    let focal: Vec<&str> = tasks
        .iter()
        .filter(|(task, _)| task.item.focal)
        .map(|(task, _)| task.item.key.as_str())
        .collect();
    let mut slots: BTreeMap<i32, i32> = BTreeMap::new();
    for line in &body.lines {
        let ((from, from_lane), (to, to_lane)) = (rects[line.from.as_str()], rects[line.to.as_str()]);
        let start = Point::new(from.right(), from.cy());
        let slot = slots.entry(from.right()).or_default();
        let mid = from.right() + GAP_COLUMN / 2 - 8 + (*slot % 3) * 8;
        *slot += 1;
        let points = if from_lane == to_lane {
            let end = Point::new(to.x, to.cy());
            if to.x > from.x {
                crate::geometry::elbow(start, end, false, mid)
            } else {
                let y = to.bottom() + LANE_PAD / 2;
                vec![
                    start,
                    Point::new(mid, start.y),
                    Point::new(mid, y),
                    Point::new(to.x - 12, y),
                    Point::new(to.x - 12, end.y),
                    end,
                ]
            }
        } else if to_lane > from_lane {
            let end = Point::new(to.cx(), to.y);
            let track = to.y - LANE_PAD / 2 - 2;
            vec![
                start,
                Point::new(mid, start.y),
                Point::new(mid, track),
                Point::new(end.x, track),
                end,
            ]
        } else {
            let end = Point::new(to.cx(), to.bottom());
            let track = to.bottom() + LANE_PAD / 2 + 2;
            vec![
                start,
                Point::new(mid, start.y),
                Point::new(mid, track),
                Point::new(end.x, track),
                end,
            ]
        };
        let column = body
            .lanes
            .iter()
            .flat_map(|lane| &lane.tasks)
            .find(|task| task.item.key == line.to)
            .map_or(0, |task| column_index[task.column.as_str()]);
        let accent = focal.contains(&line.from.as_str()) || focal.contains(&line.to.as_str());
        canvas.line(
            format!("line:{}->{}", line.from, line.to),
            2 * column + 2,
            crate::geometry::simplify(points),
            if line.trigger {
                LineKind::Optional
            } else {
                LineKind::Call
            },
            accent,
            None,
        );
    }

    for (task, lane) in &tasks {
        let (rect, _) = rects[task.item.key.as_str()];
        let mut shape = item_box(&task.item, rect);
        shape.corner = Some(body.lanes[*lane].badge.clone());
        shape.foot = task.tool.clone();
        shape.tags = match (task.input, task.output) {
            (Some(input), Some(output)) => vec![format!("{}→{}", input.code(), output.code())],
            (Some(input), None) => vec![format!("{}→", input.code())],
            (None, Some(output)) => vec![format!("→{}", output.code())],
            (None, None) => Vec::new(),
        };
        canvas.push(
            format!("task:{}", task.item.key),
            2 * column_index[task.column.as_str()] + 1,
            Shape::Box(shape),
        );
    }
}
