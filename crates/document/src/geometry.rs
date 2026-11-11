//! Sizes, grids and routes that every layout shares.
//!
//! Every position and size is a whole number on a 4px grid. Text is not
//! measured: its width is its characters times the font's advance, which is
//! wide enough for the fonts the page names.

use std::collections::BTreeMap;

use crate::scene::{Point, Rect};

pub const GRID: i32 = 4;
/// Space around the whole diagram.
pub const MARGIN: i32 = 32;
pub const BOX_MIN_W: i32 = 112;
pub const BOX_H: i32 = 44;
pub const BOX_H_SUB: i32 = 56;
pub const FOOT_H: i32 = 16;
/// How far a bend is rounded.
pub const RADIUS: i32 = 8;
/// The least room a line leaves between itself and a box it does not meet.
pub const CLEARANCE: i32 = 12;

/// `value`, rounded up to the grid.
pub fn snap(value: i32) -> i32 {
    (value + GRID - 1).div_euclid(GRID) * GRID
}

/// The width of a box's name: a sans-serif at 12px and weight 600.
pub fn name_width(text: &str) -> i32 {
    (text.chars().count() as f32 * 7.4).ceil() as i32
}

/// The width of a second line: a monospace at 10px.
pub fn mono_width(text: &str) -> i32 {
    (text.chars().count() as f32 * 6.3).ceil() as i32
}

/// The width of a line's label, with its mask: a monospace at 9px, in
/// capitals.
pub fn label_width(text: &str) -> i32 {
    snap((text.chars().count() as f32 * 6.0).ceil() as i32 + 12)
}

/// The width a box needs for its lines of text.
pub fn box_width(name: &str, sub: Option<&str>, foot: Option<&str>) -> i32 {
    let text = [name_width(name), sub.map_or(0, mono_width), foot.map_or(0, mono_width)]
        .into_iter()
        .max()
        .unwrap_or(0);
    snap((text + 32).max(BOX_MIN_W))
}

pub fn box_height(sub: bool, foot: bool) -> i32 {
    let base = if sub { BOX_H_SUB } else { BOX_H };
    if foot { base + FOOT_H } else { base }
}

/// The side of a box a line leaves or enters by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    /// The point in the middle of this side of `rect`, moved along the side
    /// by `offset`.
    pub fn point(self, rect: &Rect, offset: i32) -> Point {
        match self {
            Side::Top => Point::new(rect.cx() + offset, rect.y),
            Side::Bottom => Point::new(rect.cx() + offset, rect.bottom()),
            Side::Left => Point::new(rect.x, rect.cy() + offset),
            Side::Right => Point::new(rect.right(), rect.cy() + offset),
        }
    }

    pub fn vertical(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// Spreads the lines that meet one side of a box along it, so that no two
/// meet it at one point: the k-th of n at L·k/(n+1), at least 12 apart.
#[derive(Debug, Default)]
pub struct Ports {
    /// For each box and side, the ends there, each with the coordinate of
    /// the other end, which orders them.
    ends: BTreeMap<(String, Side), Vec<(usize, i32)>>,
}

impl Ports {
    /// Notes that line `line` meets `side` of the box `key`, and that its
    /// other end is at `toward` along that side.
    pub fn add(&mut self, key: &str, side: Side, line: usize, toward: i32) {
        self.ends
            .entry((key.to_string(), side))
            .or_default()
            .push((line, toward));
    }

    /// The lines that meet `side` of the box `key`.
    pub fn lines(&self, key: &str, side: Side) -> Vec<usize> {
        self.ends
            .get(&(key.to_string(), side))
            .map_or_else(Vec::new, |ends| ends.iter().map(|(line, _)| *line).collect())
    }

    /// The offset from the side's middle at which `line` meets it.
    pub fn offset(&self, key: &str, side: Side, line: usize, length: i32) -> i32 {
        let Some(ends) = self.ends.get(&(key.to_string(), side)) else {
            return 0;
        };
        let mut sorted = ends.clone();
        sorted.sort_by_key(|(line, toward)| (*toward, *line));
        let n = sorted.len() as i32;
        let k = sorted.iter().position(|(other, _)| *other == line).unwrap_or(0) as i32 + 1;
        let usable = (length - 2 * 12).max(0);
        let step = if n > 1 { (usable / (n + 1)).max(12) } else { 0 };
        let offset = step * k - step * (n + 1) / 2;
        offset.div_euclid(GRID) * GRID
    }
}

/// A route from `start` to `end` that leaves `start` along `out` and enters
/// `end` along `into`, turning at `mid`: the coordinate of the one segment
/// that crosses between them.
pub fn elbow(start: Point, end: Point, vertical_first: bool, mid: i32) -> Vec<Point> {
    let mut points = vec![start];
    if vertical_first {
        if start.x == end.x {
            points.push(end);
            return points;
        }
        points.push(Point::new(start.x, mid));
        points.push(Point::new(end.x, mid));
    } else {
        if start.y == end.y {
            points.push(end);
            return points;
        }
        points.push(Point::new(mid, start.y));
        points.push(Point::new(mid, end.y));
    }
    points.push(end);
    simplify(points)
}

/// A route between two ranks that may have ranks between them, as a
/// dependency or an architecture diagram draws one. It leaves `start`
/// along the flow, turns in the channel at `leave` just past the rank it
/// leaves, runs along a lane across the ranks between, turns again in the
/// channel at `arrive` just before the rank it enters, and enters `end`
/// along the flow. Of every lane beside a box, it takes the one that passes
/// behind the fewest of `boxes`, or comes within `clear` of the fewest, and
/// then the shortest.
pub fn across(start: Point, end: Point, down: bool, leave: i32, arrive: i32, boxes: &[Rect], clear: i32) -> Vec<Point> {
    if !down {
        let flip = |p: Point| Point::new(p.y, p.x);
        let flipped: Vec<Rect> = boxes.iter().map(|r| Rect::new(r.y, r.x, r.h, r.w)).collect();
        return across(flip(start), flip(end), true, leave, arrive, &flipped, clear)
            .into_iter()
            .map(flip)
            .collect();
    }
    let mut lanes = vec![start.x, end.x];
    for rect in boxes {
        lanes.push(rect.x - 16);
        lanes.push(rect.right() + 16);
    }
    let near: Vec<Rect> = boxes.iter().map(|rect| rect.grow(clear)).collect();
    let mut candidates = vec![elbow(start, end, true, leave), elbow(start, end, true, arrive)];
    for lane in lanes {
        candidates.push(simplify(vec![
            start,
            Point::new(start.x, leave),
            Point::new(lane, leave),
            Point::new(lane, arrive),
            Point::new(end.x, arrive),
            end,
        ]));
    }
    candidates
        .into_iter()
        .min_by_key(|route| {
            let length: i32 = route
                .windows(2)
                .map(|s| (s[0].x - s[1].x).abs() + (s[0].y - s[1].y).abs())
                .sum();
            (crossings(route, &near), route.len(), length)
        })
        .expect("there is always a route")
}

/// The same route without repeated points or points in the middle of a
/// straight run.
pub fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        if out.last() == Some(&point) {
            continue;
        }
        if out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            if (a.x == b.x && b.x == point.x) || (a.y == b.y && b.y == point.y) {
                out.pop();
            }
        }
        out.push(point);
    }
    out
}

/// How many of `boxes` a route passes through.
pub fn crossings(points: &[Point], boxes: &[Rect]) -> usize {
    points
        .windows(2)
        .map(|segment| {
            boxes
                .iter()
                .filter(|rect| rect.crossed_by(segment[0], segment[1]))
                .count()
        })
        .sum()
}

/// How many times two routes meet: each point where a segment of one
/// crosses or touches a segment of the other, and each stretch they run
/// along together.
pub fn meetings(a: &[Point], b: &[Point]) -> usize {
    let span = |p: i32, q: i32| (p.min(q), p.max(q));
    let mut points: Vec<(i32, i32)> = Vec::new();
    let mut runs = 0;
    for s in a.windows(2) {
        for t in b.windows(2) {
            let (s_flat, t_flat) = (s[0].y == s[1].y, t[0].y == t[1].y);
            if s_flat == t_flat {
                // Parallel: they meet only on one line.
                let (on, (s0, s1), (t0, t1)) = if s_flat {
                    (s[0].y == t[0].y, span(s[0].x, s[1].x), span(t[0].x, t[1].x))
                } else {
                    (s[0].x == t[0].x, span(s[0].y, s[1].y), span(t[0].y, t[1].y))
                };
                let (from, to) = (s0.max(t0), s1.min(t1));
                if !on || from > to {
                    continue;
                }
                if from < to {
                    runs += 1;
                } else if s_flat {
                    points.push((from, s[0].y));
                } else {
                    points.push((s[0].x, from));
                }
            } else {
                let (flat, upright) = if s_flat { (s, t) } else { (t, s) };
                let (x0, x1) = span(flat[0].x, flat[1].x);
                let (y0, y1) = span(upright[0].y, upright[1].y);
                let (x, y) = (upright[0].x, flat[0].y);
                if (x0..=x1).contains(&x) && (y0..=y1).contains(&y) {
                    points.push((x, y));
                }
            }
        }
    }
    points.sort_unstable();
    points.dedup();
    points.len() + runs
}

/// The point halfway along the longest segment of a route, for its label.
pub fn label_point(points: &[Point]) -> Point {
    let segment = points
        .windows(2)
        .max_by_key(|segment| (segment[0].x - segment[1].x).abs() + (segment[0].y - segment[1].y).abs())
        .expect("a route has a segment");
    Point::new((segment[0].x + segment[1].x) / 2, (segment[0].y + segment[1].y) / 2)
}

/// The point for a label `width` wide on a route that passes `boxes`: the
/// middle of the longest stretch along which the label's mask keeps half
/// the [`CLEARANCE`] from every box, or [`label_point`] when there is none.
pub fn label_point_clear(points: &[Point], width: i32, boxes: &[Rect]) -> Point {
    let room = CLEARANCE / 2;
    let mut best: Option<(i32, Point)> = None;
    for segment in points.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        let flat = a.y == b.y;
        let (from, to) = if flat {
            (a.x.min(b.x), a.x.max(b.x))
        } else {
            (a.y.min(b.y), a.y.max(b.y))
        };
        // The stretch of this segment each box keeps the label off.
        let mut blocked: Vec<(i32, i32)> = boxes
            .iter()
            .filter_map(|rect| {
                if flat {
                    let beside = a.y - 8 < rect.bottom() + room && a.y + 8 > rect.y - room;
                    beside.then_some((rect.x - room - width / 2, rect.right() + room + width / 2))
                } else {
                    let beside = a.x - width / 2 < rect.right() + room && a.x + width / 2 > rect.x - room;
                    beside.then_some((rect.y - room - 8, rect.bottom() + room + 8))
                }
            })
            .collect();
        blocked.sort_unstable();
        let mut at = from;
        let mut free = Vec::new();
        for (lo, hi) in blocked {
            if lo > at {
                free.push((at, lo.min(to)));
            }
            at = at.max(hi);
        }
        if at < to {
            free.push((at, to));
        }
        for (lo, hi) in free {
            let mid = (lo + hi) / 2;
            let point = if flat {
                Point::new(mid, a.y)
            } else {
                Point::new(a.x, mid)
            };
            if best.is_none_or(|(length, _)| hi - lo > length) {
                best = Some((hi - lo, point));
            }
        }
    }
    best.map_or_else(|| label_point(points), |(_, point)| point)
}

/// An SVG path through `points`, each bend rounded by a quarter circle of
/// [`RADIUS`], or less where a segment is short.
pub fn path(points: &[Point], dx: i32, dy: i32) -> String {
    let p = |point: Point| (point.x - dx, point.y - dy);
    let mut d = String::new();
    let (x0, y0) = p(points[0]);
    d.push_str(&format!("M {x0} {y0}"));
    for i in 1..points.len() {
        let (x, y) = p(points[i]);
        if i + 1 < points.len() {
            let (px, py) = p(points[i - 1]);
            let (nx, ny) = p(points[i + 1]);
            let before = ((x - px).abs() + (y - py).abs()) / 2;
            let after = ((nx - x).abs() + (ny - y).abs()) / 2;
            let r = RADIUS.min(before).min(after);
            let (ix, iy) = (x - (x - px).signum() * r, y - (y - py).signum() * r);
            let (ox, oy) = (x + (nx - x).signum() * r, y + (ny - y).signum() * r);
            d.push_str(&format!(" L {ix} {iy} Q {x} {y} {ox} {oy}"));
        } else {
            d.push_str(&format!(" L {x} {y}"));
        }
    }
    d
}

/// The arrowhead at the end of a route: its tip on the last point, pointing
/// along the last segment.
pub fn head(points: &[Point], dx: i32, dy: i32, open: bool) -> String {
    let tip = points[points.len() - 1];
    let before = points[points.len() - 2];
    let (ux, uy) = ((tip.x - before.x).signum(), (tip.y - before.y).signum());
    let (tx, ty) = (tip.x - dx, tip.y - dy);
    let (bx, by) = (tx - ux * 8, ty - uy * 8);
    let (px, py) = (-uy * 4, ux * 4);
    if open {
        format!("M {} {} L {tx} {ty} L {} {}", bx + px, by + py, bx - px, by - py)
    } else {
        format!("M {} {} L {tx} {ty} L {} {} Z", bx + px, by + py, bx - px, by - py)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_snaps_up_to_the_grid() {
        assert_eq!(snap(0), 0);
        assert_eq!(snap(1), 4);
        assert_eq!(snap(8), 8);
        assert_eq!(snap(-3), 0);
    }

    #[test]
    fn an_elbow_turns_twice_and_a_straight_route_not_at_all() {
        let bent = elbow(Point::new(0, 0), Point::new(40, 80), true, 40);
        assert_eq!(
            bent,
            vec![
                Point::new(0, 0),
                Point::new(0, 40),
                Point::new(40, 40),
                Point::new(40, 80)
            ]
        );
        let straight = elbow(Point::new(0, 0), Point::new(0, 80), true, 40);
        assert_eq!(straight, vec![Point::new(0, 0), Point::new(0, 80)]);
    }

    #[test]
    fn a_bend_is_rounded_by_a_quarter_circle() {
        let d = path(&[Point::new(0, 0), Point::new(0, 40), Point::new(40, 40)], 0, 0);
        assert_eq!(d, "M 0 0 L 0 32 Q 0 40 8 40 L 40 40");
    }

    #[test]
    fn two_routes_meet_where_they_cross_and_where_they_run_together() {
        let p = |x, y| Point::new(x, y);
        let z = [p(0, 0), p(20, 0), p(20, 40), p(60, 40)];
        assert_eq!(meetings(&z, &[p(0, 20), p(60, 20)]), 1);
        assert_eq!(meetings(&z, &[p(0, 60), p(60, 60)]), 0);
        assert_eq!(meetings(&z, &[p(30, 40), p(80, 40)]), 1, "a shared stretch");
        assert_eq!(meetings(&z, &[p(0, 10), p(40, 10), p(40, 60)]), 2);
    }

    #[test]
    fn ports_spread_the_ends_on_one_side() {
        let mut ports = Ports::default();
        for line in 0..3 {
            ports.add("a", Side::Bottom, line, line as i32 * 10);
        }
        let offsets: Vec<i32> = (0..3).map(|line| ports.offset("a", Side::Bottom, line, 160)).collect();
        assert!(offsets.windows(2).all(|pair| pair[1] - pair[0] >= 12), "{offsets:?}");
        assert_eq!(ports.offset("a", Side::Top, 0, 160), 0);
    }
}
