//! The complete document of a diagram: every node it draws, placed and
//! styled, and how each moves over time.
//!
//! Nothing is left to decide once a document is made. Every colour, border,
//! radius and font is resolved, and every animation is a track of
//! keyframes. A writer only translates: `diagram-html` writes each node as a
//! `<div>` with an inline style, and `diagram-render` builds the same nodes
//! for takumi. So a page and a picture of one document show the same thing.
//!
//! A node is a box: absolutely placed in the diagram, or placed by its
//! parent's flex layout. It has a background, a border on each side, a
//! radius at each corner, padding, an opacity and a rotation, and holds
//! either text or other nodes. Lines are drawn with boxes too: a segment is
//! a box with one border, a rounded bend a box with two borders and one
//! rounded corner, an arrowhead a box whose borders meet in a triangle.

use crate::scene::Rect;

/// A colour, with its opacity from 0 to 255.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba(0, 0, 0, 0);

    /// `#rrggbb`, opaque.
    pub const fn hex(rgb: u32) -> Rgba {
        Rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
    }

    /// The same colour at `alpha`, from 0 to 1.
    pub fn alpha(self, alpha: f32) -> Rgba {
        Rgba(self.0, self.1, self.2, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub width: i32,
    pub height: i32,
    pub title: String,
    pub desc: String,
    /// How long its animation runs; 0 when it does not move.
    pub duration_ms: u32,
    /// What a diff changed, when the document is a diff.
    pub changes: Option<Changes>,
    /// Drawn in order: a later node over an earlier one.
    pub nodes: Vec<Node>,
    /// The animations nodes refer to by index.
    pub tracks: Vec<Track>,
}

/// How many elements a diff added, removed and changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Changes {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The element of the scene it draws, the same in every version of the
    /// diagram: `box:cli`.
    pub id: Option<String>,
    /// The pile a box stands for, as the file named it.
    pub pile: Option<String>,
    pub place: Place,
    pub style: Style,
    /// The track that moves it, by index into [`Document::tracks`].
    pub track: Option<usize>,
    pub content: Content,
}

impl Node {
    pub fn new(place: Place, style: Style) -> Node {
        Node {
            id: None,
            pile: None,
            place,
            style,
            track: None,
            content: Content::None,
        }
    }

    pub fn absolute(rect: Rect, style: Style) -> Node {
        Node::new(Place::Absolute(rect), style)
    }

    pub fn with_children(mut self, children: Vec<Node>) -> Node {
        self.content = Content::Children(children);
        self
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Node {
        self.id = Some(id.into());
        self
    }

    pub fn with_track(mut self, track: Option<usize>) -> Node {
        self.track = track;
        self
    }

    /// A line of text, placed by its parent's flex layout.
    pub fn text(text: Text) -> Node {
        let mut node = Node::new(Place::Flow, Style::default());
        node.content = Content::Text(text);
        node
    }

    pub fn children(&self) -> &[Node] {
        match &self.content {
            Content::Children(children) => children,
            _ => &[],
        }
    }

    /// This node and every node inside it, this one first.
    pub fn walk(&self) -> Vec<&Node> {
        let mut all = vec![self];
        for child in self.children() {
            all.extend(child.walk());
        }
        all
    }
}

/// Where a node is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Place {
    /// At these coordinates and this size: within the diagram for a node of
    /// the document, within its parent for a child.
    Absolute(Rect),
    /// Where its parent's flex layout puts it, at the size of its content.
    Flow,
    /// Where its parent's flex layout puts it, at this size.
    Sized { w: i32, h: i32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    None,
    Text(Text),
    Children(Vec<Node>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
}

/// One side of a border.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub width: f32,
    pub style: LineStyle,
    pub color: Rgba,
}

impl Edge {
    pub fn solid(width: f32, color: Rgba) -> Edge {
        Edge {
            width,
            style: LineStyle::Solid,
            color,
        }
    }

    pub fn dashed(width: f32, color: Rgba) -> Edge {
        Edge {
            width,
            style: LineStyle::Dashed,
            color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// How a node lays out its children: a flex row or column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flex {
    pub column: bool,
    /// Across the direction.
    pub align: Align,
    /// Along the direction.
    pub justify: Align,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    pub background: Option<Rgba>,
    /// Top, right, bottom, left.
    pub border: [Option<Edge>; 4],
    /// Top left, top right, bottom right, bottom left.
    pub radius: [f32; 4],
    /// Top, right, bottom, left.
    pub padding: [i32; 4],
    pub flex: Option<Flex>,
    /// From 0 to 1; `None` is fully opaque.
    pub opacity: Option<f32>,
    /// Degrees, clockwise, about the node's centre.
    pub rotate: Option<f32>,
    /// Whatever is drawn outside the node is cut off.
    pub clip: bool,
}

impl Style {
    pub fn border_all(mut self, edge: Edge) -> Style {
        self.border = [Some(edge); 4];
        self
    }

    pub fn radius_all(mut self, radius: f32) -> Style {
        self.radius = [radius; 4];
        self
    }

    pub fn background(mut self, color: Rgba) -> Style {
        self.background = Some(color);
        self
    }

    pub fn padding(mut self, vertical: i32, horizontal: i32) -> Style {
        self.padding = [vertical, horizontal, vertical, horizontal];
        self
    }

    pub fn flex(mut self, column: bool, align: Align, justify: Align) -> Style {
        self.flex = Some(Flex { column, align, justify });
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Style {
        self.opacity = Some(opacity);
        self
    }
}

/// The two families a diagram's text is set in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    /// Geist: names and headings.
    Sans,
    /// Geist Mono: second lines, labels and tags.
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
}

/// A run of text on one line: it never wraps.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub value: String,
    pub family: Family,
    /// Pixels.
    pub size: f32,
    pub weight: u16,
    pub color: Rgba,
    /// Pixels between letters.
    pub letter_spacing: f32,
    /// Pixels.
    pub line_height: f32,
    pub align: TextAlign,
    pub strike: bool,
}

impl Text {
    pub fn new(value: impl Into<String>, family: Family, size: f32, color: Rgba) -> Text {
        Text {
            value: value.into(),
            family,
            size,
            weight: 400,
            color,
            letter_spacing: 0.0,
            line_height: (size * 1.35).round(),
            align: TextAlign::Left,
            strike: false,
        }
    }

    pub fn weight(mut self, weight: u16) -> Text {
        self.weight = weight;
        self
    }

    pub fn spacing(mut self, letter_spacing: f32) -> Text {
        self.letter_spacing = letter_spacing;
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Text {
        self.line_height = line_height;
        self
    }

    pub fn centered(mut self) -> Text {
        self.align = TextAlign::Center;
        self
    }
}

/// How fast a change runs between two keyframes: the CSS timing functions,
/// as the cubic Bézier curves CSS defines them by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ease {
    Linear,
    In,
    Out,
    InOut,
}

impl Ease {
    /// The curve's control points, as CSS's `cubic-bezier()` takes them.
    pub fn bezier(self) -> (f32, f32, f32, f32) {
        match self {
            Ease::Linear => (0.0, 0.0, 1.0, 1.0),
            Ease::In => (0.42, 0.0, 1.0, 1.0),
            Ease::Out => (0.0, 0.0, 0.58, 1.0),
            Ease::InOut => (0.42, 0.0, 0.58, 1.0),
        }
    }

    /// How far the change has gone at `progress` of its time, both from 0
    /// to 1.
    pub fn apply(self, progress: f32) -> f32 {
        let (x1, y1, x2, y2) = self.bezier();
        if self == Ease::Linear {
            return progress;
        }
        let bezier = |t: f32, a: f32, b: f32| {
            let u = 1.0 - t;
            3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
        };
        // The curve's parameter at which x is `progress`, by bisection.
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..40 {
            let mid = (lo + hi) / 2.0;
            if bezier(mid, x1, x2) < progress {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        bezier((lo + hi) / 2.0, y1, y2)
    }
}

/// Where a node is moved and how opaque it is, at a moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub opacity: f32,
    pub dx: f32,
    pub dy: f32,
}

impl Pose {
    pub const REST: Pose = Pose {
        opacity: 1.0,
        dx: 0.0,
        dy: 0.0,
    };
}

/// A keyframe: the pose at `at_ms`, reached from the keyframe before it
/// with `ease`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub at_ms: u32,
    pub pose: Pose,
    pub ease: Ease,
}

/// How a node moves over the document's whole animation. Before the first
/// key it holds the first pose, and after the last key the last.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub keys: Vec<Key>,
}

impl Track {
    /// From `from` until `start_ms`, then to `to` over `length_ms`.
    pub fn change(from: Pose, to: Pose, start_ms: u32, length_ms: u32, ease: Ease) -> Track {
        Track {
            keys: vec![
                Key {
                    at_ms: start_ms,
                    pose: from,
                    ease: Ease::Linear,
                },
                Key {
                    at_ms: start_ms + length_ms,
                    pose: to,
                    ease,
                },
            ],
        }
    }

    pub fn at(&self, time_ms: u32) -> Pose {
        let Some(first) = self.keys.first() else {
            return Pose::REST;
        };
        if time_ms <= first.at_ms {
            return first.pose;
        }
        for pair in self.keys.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if time_ms <= b.at_ms {
                let span = (b.at_ms - a.at_ms).max(1) as f32;
                let t = b.ease.apply((time_ms - a.at_ms) as f32 / span);
                let mix = |x: f32, y: f32| x + (y - x) * t;
                return Pose {
                    opacity: mix(a.pose.opacity, b.pose.opacity),
                    dx: mix(a.pose.dx, b.pose.dx),
                    dy: mix(a.pose.dy, b.pose.dy),
                };
            }
        }
        self.keys[self.keys.len() - 1].pose
    }
}

impl Document {
    /// The pose of `node` at `time_ms`.
    pub fn pose(&self, node: &Node, time_ms: u32) -> Pose {
        node.track
            .and_then(|track| self.tracks.get(track))
            .map_or(Pose::REST, |track| track.at(time_ms))
    }

    /// Every node, with every node inside it, in drawing order.
    pub fn walk(&self) -> Vec<&Node> {
        self.nodes.iter().flat_map(Node::walk).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ease_runs_from_nothing_to_all_of_the_change() {
        for ease in [Ease::Linear, Ease::In, Ease::Out, Ease::InOut] {
            assert!(ease.apply(0.0).abs() < 1e-3, "{ease:?}");
            assert!((ease.apply(1.0) - 1.0).abs() < 1e-3, "{ease:?}");
        }
        assert!(Ease::Out.apply(0.5) > 0.5, "an ease out runs fast first");
        assert!(Ease::In.apply(0.5) < 0.5, "an ease in runs slow first");
        assert!((Ease::InOut.apply(0.5) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn a_track_holds_before_its_first_key_and_after_its_last() {
        let hidden = Pose {
            opacity: 0.0,
            ..Pose::REST
        };
        let track = Track::change(hidden, Pose::REST, 100, 200, Ease::Linear);
        assert_eq!(track.at(0).opacity, 0.0);
        assert_eq!(track.at(100).opacity, 0.0);
        assert!((track.at(200).opacity - 0.5).abs() < 1e-3);
        assert_eq!(track.at(300).opacity, 1.0);
        assert_eq!(track.at(10_000).opacity, 1.0);
    }
}
