//! A diagram laid out: every element placed, with nothing left to decide but
//! how it looks.
//!
//! A layout makes a scene from a diagram, and the HTML writer draws it. Each
//! element has an id that stays the same between two versions of a diagram
//! (`box:cli`, `line:cli->storage`), so two scenes can be compared element
//! by element.

use diagram_format::Look;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn cx(&self) -> i32 {
        self.x + self.w / 2
    }
    pub fn cy(&self) -> i32 {
        self.y + self.h / 2
    }
    pub fn translate(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
    /// This rectangle grown by `by` on every side.
    pub fn grow(&self, by: i32) -> Rect {
        Rect::new(self.x - by, self.y - by, self.w + 2 * by, self.h + 2 * by)
    }
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }
    /// Whether this rectangle and `other` share any of their inside.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.right() && other.x < self.right() && self.y < other.bottom() && other.y < self.bottom()
    }
    /// Whether the segment from `a` to `b`, horizontal or vertical, passes
    /// through the inside of this rectangle.
    pub fn crossed_by(&self, a: Point, b: Point) -> bool {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        x1 > self.x && x0 < self.right() && y1 > self.y && y0 < self.bottom()
    }
}

/// How an element changed between two versions of a diagram, as a report
/// says it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Same,
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: i32,
    pub height: i32,
    pub title: String,
    pub desc: String,
    pub note: Option<String>,
    pub elements: Vec<Element>,
}

impl Scene {
    pub fn element(&self, id: &str) -> Option<&Element> {
        self.elements.iter().find(|element| element.id == id)
    }

    /// How many elements a report names by `subject`.
    pub fn count(&self, subject: &crate::Subject) -> usize {
        self.elements
            .iter()
            .filter(|element| element.shape.subject().as_ref() == Some(subject))
            .count()
    }

    /// The last step of the reveal, for its timing.
    pub fn last_step(&self) -> u32 {
        self.elements.iter().map(|element| element.step).max().unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// The same in every version of the diagram, for the same element.
    pub id: String,
    /// When it appears in a reveal: elements of one step appear together.
    pub step: u32,
    pub status: Status,
    pub shape: Shape,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Box(BoxShape),
    Line(LineShape),
    Region(RegionShape),
    Text(TextShape),
    Dot(DotShape),
}

impl Shape {
    /// Where the shape is drawn.
    pub fn bounds(&self) -> Rect {
        match self {
            Shape::Box(shape) => shape.rect,
            Shape::Region(shape) => shape.rect,
            Shape::Text(shape) => Rect::new(shape.at.x, shape.at.y, shape.width, 16),
            Shape::Dot(shape) => Rect::new(shape.center.x - 8, shape.center.y - 8, 16, 16),
            Shape::Line(shape) => {
                let mut bounds = Rect::new(shape.points[0].x, shape.points[0].y, 0, 0);
                for point in &shape.points {
                    bounds = bounds.union(&Rect::new(point.x, point.y, 0, 0));
                }
                if let Some(label) = &shape.label {
                    bounds = bounds.union(&label.rect());
                }
                bounds
            }
        }
    }

    /// What the shape draws, without where it is: two shapes with the same
    /// signature are drawn the same, wherever each is. Metadata is no part
    /// of it: nothing here reads metadata, and none changes a picture.
    pub fn signature(&self) -> String {
        match self {
            Shape::Box(shape) => {
                let said = shape.value.as_ref().or(shape.foot.as_ref());
                format!(
                    "box|{}|{:?}|{:?}|{}|{:?}|{:?}|{:?}",
                    shape.name, shape.sub, shape.look, shape.focal, shape.corner, said, shape.tags
                )
            }
            Shape::Line(shape) => format!(
                "line|{:?}|{}|{:?}|{:?}",
                shape.head,
                shape.dashed,
                shape.tone,
                shape.label.as_ref().map(|label| &label.text)
            ),
            Shape::Region(shape) => format!(
                "region|{:?}|{}|{}|{:?}",
                shape.label, shape.dashed, shape.focal, shape.kind
            ),
            Shape::Text(shape) => format!("text|{}|{:?}", shape.text, shape.style),
            Shape::Dot(shape) => format!("dot|{}", shape.ring),
        }
    }

    /// What a report names the shape by: the key the file names an
    /// element by, or the keys of the two elements a line joins. None for a
    /// shape the file names by no key, which a report names by its id.
    pub fn subject(&self) -> Option<crate::Subject> {
        match self {
            Shape::Box(shape) => shape.key.clone().map(crate::Subject::Element),
            Shape::Region(shape) => shape.key.clone().map(crate::Subject::Element),
            Shape::Line(shape) => shape.ends.clone().map(|(from, to)| crate::Subject::Line { from, to }),
            _ => None,
        }
    }

    pub fn translate(&mut self, dx: i32, dy: i32) {
        match self {
            Shape::Box(shape) => shape.rect = shape.rect.translate(dx, dy),
            Shape::Region(shape) => shape.rect = shape.rect.translate(dx, dy),
            Shape::Text(shape) => shape.at = Point::new(shape.at.x + dx, shape.at.y + dy),
            Shape::Dot(shape) => shape.center = Point::new(shape.center.x + dx, shape.center.y + dy),
            Shape::Line(shape) => {
                for point in &mut shape.points {
                    *point = Point::new(point.x + dx, point.y + dy);
                }
                if let Some(label) = &mut shape.label {
                    label.at = Point::new(label.at.x + dx, label.at.y + dy);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoxShape {
    pub rect: Rect,
    pub name: String,
    pub sub: Option<String>,
    pub look: Look,
    pub focal: bool,
    /// A small tag at the top right: how many depend on it, a lane's badge.
    pub corner: Option<String>,
    /// A third line: the tool a task is done with, a part's value.
    pub foot: Option<String>,
    /// Small tags at the bottom right: the data a task takes and gives.
    pub tags: Vec<String>,
    /// How strongly the box is filled, from 0 to 1: a treemap's larger parts.
    pub fill: Option<f32>,
    /// Text at the top left instead of the middle: a box that holds boxes.
    pub top: bool,
    /// The key of the element the box draws, when the file names it by
    /// one.
    pub key: Option<String>,
    /// The element's metadata, as [`metadata`] gives it.
    pub meta: Vec<(String, String)>,
    /// What the file says the box is worth, when its foot is drawn from
    /// it: a treemap part's value, whose share of the whole is not what the
    /// file says, and changes when another part changes.
    pub value: Option<String>,
}

impl BoxShape {
    pub fn new(rect: Rect, name: impl Into<String>) -> BoxShape {
        BoxShape {
            rect,
            name: name.into(),
            sub: None,
            look: Look::Plain,
            focal: false,
            corner: None,
            foot: None,
            tags: Vec::new(),
            fill: None,
            top: false,
            key: None,
            meta: Vec::new(),
            value: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Head {
    /// A filled arrowhead.
    Arrow,
    /// An open arrowhead, for a message nobody waits for.
    Open,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Ink,
    Muted,
    Link,
    Accent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineShape {
    /// At least two; every segment horizontal or vertical.
    pub points: Vec<Point>,
    pub head: Head,
    pub dashed: bool,
    pub tone: Tone,
    pub label: Option<Label>,
    /// The keys of the two elements it joins, from and to, when it draws a
    /// line the file may give metadata: a `line`, or a `cycle`.
    pub ends: Option<(String, String)>,
    /// The line's metadata, as [`metadata`] gives it.
    pub meta: Vec<(String, String)>,
}

/// An element's or a line's metadata as a scene carries it: each property
/// as `<owner>-<key>` and its value as text, in the order the file wrote
/// them. A scene carries metadata and never reads it.
pub fn metadata(meta: &[diagram_format::Meta]) -> Vec<(String, String)> {
    meta.iter()
        .map(|meta| (format!("{}-{}", meta.owner, meta.key), meta.value.to_string()))
        .collect()
}

/// A line's label, in capitals, on a mask of the paper colour.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub text: String,
    /// Its centre.
    pub at: Point,
}

impl Label {
    pub fn rect(&self) -> Rect {
        let w = crate::geometry::label_width(&self.text);
        Rect::new(self.at.x - w / 2, self.at.y - 8, w, 16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// A boundary around a tier.
    Zone,
    /// A lane of a swimlane or a process.
    Lane,
    /// A scope of a nested diagram.
    Level,
    /// An opt, loop or alt of a sequence.
    Fragment,
    /// A step of a process, as a column.
    Column,
    /// The bar on a lifeline while an actor holds control.
    Activation,
    /// An actor's lifeline.
    Lifeline,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegionShape {
    pub rect: Rect,
    pub kind: RegionKind,
    /// Drawn at the top left, in capitals.
    pub label: Option<String>,
    /// A second line beside the label.
    pub sub: Option<String>,
    pub dashed: bool,
    pub focal: bool,
    /// The key of the element the region draws, when the file names it
    /// by one: a nested diagram's level.
    pub key: Option<String>,
    /// The element's metadata, as [`metadata`] gives it.
    pub meta: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    /// Small capitals in monospace: a column's or a fragment's label.
    Eyebrow,
    /// A sentence, muted.
    Note,
    /// A lane's name.
    Name,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextShape {
    /// Its top left.
    pub at: Point,
    pub width: i32,
    pub text: String,
    pub style: TextStyle,
    pub focal: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DotShape {
    pub center: Point,
    /// A ringed dot, where a state machine ends; a filled one, where it starts.
    pub ring: bool,
}
