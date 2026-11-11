//! How one whole splits into parts. Each part is a rectangle whose area is
//! its share of the whole, laid out so each is near square (the squarified
//! treemap of Bruls, Huizing and van Wijk), which is what lets the eye
//! compare two of them. Larger parts are filled stronger. A part too small
//! for its name shows its short name, and is named in full under the map.

use diagram_format::Treemap;

use super::{Canvas, item_box};
use crate::geometry::{mono_width, name_width};
use crate::scene::*;

const W: f64 = 640.0;
const H: f64 = 400.0;
const GAP: i32 = 4;

pub fn layout(body: &Treemap, canvas: &mut Canvas) {
    let total: f64 = body.parts.iter().map(|part| part.value).sum();
    let mut order: Vec<usize> = (0..body.parts.len()).collect();
    order.sort_by(|a, b| body.parts[*b].value.total_cmp(&body.parts[*a].value).then(a.cmp(b)));
    let areas: Vec<f64> = order
        .iter()
        .map(|index| body.parts[*index].value / total * W * H)
        .collect();
    let cells = squarify(&areas, (0.0, 0.0, W, H));

    let mut legend = Vec::new();
    for (rank, (index, cell)) in order.iter().zip(&cells).enumerate() {
        let part = &body.parts[*index];
        // From rounded edges, so neighbours share them, less the gap.
        let (x0, y0) = (cell.0.round() as i32, cell.1.round() as i32);
        let (x1, y1) = ((cell.0 + cell.2).round() as i32, (cell.1 + cell.3).round() as i32);
        let rect = Rect::new(x0, y0, (x1 - x0 - GAP).max(2), (y1 - y0 - GAP).max(2));
        let share = part.value / total * 100.0;
        let mut shape = item_box(&part.item, rect);
        shape.top = true;
        // The largest part is filled strongest, the smallest faintest.
        let strength = if order.len() > 1 {
            0.85 - 0.6 * rank as f32 / (order.len() - 1) as f32
        } else {
            0.85
        };
        shape.fill = Some(strength);
        shape.foot = Some(format!("{} · {share:.0}%", number(part.value)));
        shape.value = Some(number(part.value));
        let fits = rect.w >= name_width(&part.item.name) + 16 && rect.h >= 52;
        if !fits {
            let short = part.short.clone().unwrap_or_else(|| {
                part.item
                    .name
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .take(3)
                    .collect::<String>()
                    .to_uppercase()
            });
            shape.sub = None;
            if rect.w >= name_width(&short) + 12 && rect.h >= 24 {
                legend.push(format!("{short} {}", part.item.name));
                shape.name = short;
            } else {
                legend.push(format!("{} ({})", part.item.name, number(part.value)));
                shape.name = String::new();
            }
            if rect.h < 52 || rect.w < mono_width(shape.foot.as_deref().unwrap_or("")) + 16 {
                shape.foot = None;
            }
        }
        canvas.push(format!("part:{}", part.item.key), rank as u32, Shape::Box(shape));
    }

    let mut y = H as i32 + 12;
    let mut lines = vec![format!("AREA: {}", body.measure.to_uppercase())];
    if !legend.is_empty() {
        lines.push(legend.join(" · ").to_uppercase());
    }
    lines.push(format!("SOURCE: {}", body.source.to_uppercase()));
    let last = body.parts.len() as u32;
    for (index, text) in lines.into_iter().enumerate() {
        canvas.push(
            format!("caption:{index}"),
            last,
            Shape::Text(TextShape {
                at: Point::new(0, y),
                width: W as i32,
                text,
                style: TextStyle::Eyebrow,
                focal: false,
            }),
        );
        y += 18;
    }
}

/// A number as a part's value is drawn: whole when it is whole.
fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

type Cell = (f64, f64, f64, f64);

/// Lays `areas`, largest first, into `bounds` as rows of near-square cells.
fn squarify(areas: &[f64], bounds: Cell) -> Vec<Cell> {
    let mut cells = Vec::with_capacity(areas.len());
    let (mut x, mut y, mut w, mut h) = bounds;
    let mut start = 0;
    while start < areas.len() {
        let side = w.min(h);
        let mut end = start + 1;
        while end < areas.len() && worst(&areas[start..=end], side) <= worst(&areas[start..end], side) {
            end += 1;
        }
        let row = &areas[start..end];
        let sum: f64 = row.iter().sum();
        if w >= h {
            // A column at the left, as wide as its area needs.
            let column = sum / h;
            let mut cy = y;
            for area in row {
                let ch = area / column;
                cells.push((x, cy, column, ch));
                cy += ch;
            }
            x += column;
            w -= column;
        } else {
            let row_h = sum / w;
            let mut cx = x;
            for area in row {
                let cw = area / row_h;
                cells.push((cx, y, cw, row_h));
                cx += cw;
            }
            y += row_h;
            h -= row_h;
        }
        start = end;
    }
    cells
}

/// The worst aspect ratio of a row of `areas` laid along a side of `side`.
fn worst(areas: &[f64], side: f64) -> f64 {
    let sum: f64 = areas.iter().sum();
    let max = areas.iter().copied().fold(f64::MIN, f64::max);
    let min = areas.iter().copied().fold(f64::MAX, f64::min);
    let s2 = side * side;
    (s2 * max / (sum * sum)).max(sum * sum / (s2 * min))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cells_cover_the_whole_by_their_shares() {
        let areas = [6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0];
        let total: f64 = areas.iter().sum();
        let scaled: Vec<f64> = areas.iter().map(|a| a / total * 600.0 * 400.0).collect();
        let cells = squarify(&scaled, (0.0, 0.0, 600.0, 400.0));
        for (area, cell) in scaled.iter().zip(&cells) {
            assert!((cell.2 * cell.3 - area).abs() < 1e-6);
            assert!(cell.0 >= -1e-9 && cell.1 >= -1e-9);
            assert!(cell.0 + cell.2 <= 600.0 + 1e-6 && cell.1 + cell.3 <= 400.0 + 1e-6);
        }
    }
}
