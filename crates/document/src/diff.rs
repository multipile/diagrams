//! Two versions of a diagram, element by element.
//!
//! Elements are matched by id, which a layout gives from what the diagram
//! names: `box:cli` is the box with the key `cli` in both versions. An
//! element in the second version only was added, and one in the first only
//! was removed. An element in both changed when what it says changed, its
//! signature: its name, its look, its label. Where it is drawn is not a
//! change: an element that only moved is drawn moving, and is not marked.

use crate::scene::*;

/// What an element does as one version becomes the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The same, in the same place.
    Kept,
    /// The same, elsewhere: it slides from `dx, dy` away to where it is now.
    Moved {
        dx: i32,
        dy: i32,
    },
    Added,
    Removed,
    /// The first version of an element that changed, or changed its shape,
    /// fading out as its second version fades in.
    Old,
    /// The second version of such an element.
    New,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffElement {
    pub element: Element,
    pub role: Role,
}

/// The second version's scene, and every element of both, each with its
/// role.
#[derive(Debug, Clone, PartialEq)]
pub struct Diff {
    pub scene: Scene,
    pub elements: Vec<DiffElement>,
}

impl Diff {
    /// How many elements have `status`.
    pub fn count(&self, status: Status) -> usize {
        self.elements
            .iter()
            .filter(|item| item.role != Role::Old && item.element.status == status)
            .count()
    }
}

pub fn diff(before: &Scene, after: &Scene) -> Diff {
    let mut elements = Vec::new();
    for element in &after.elements {
        let Some(old) = before.element(&element.id) else {
            elements.push(DiffElement {
                element: Element {
                    status: Status::Added,
                    ..element.clone()
                },
                role: Role::Added,
            });
            continue;
        };
        let (was, is) = (old.shape.bounds(), element.shape.bounds());
        let changed = old.shape.signature() != element.shape.signature();
        let reshaped = match (&old.shape, &element.shape) {
            (Shape::Line(a), Shape::Line(b)) => {
                let moved_by = (b.points[0].x - a.points[0].x, b.points[0].y - a.points[0].y);
                let same_route = a.points.len() == b.points.len()
                    && a.points
                        .iter()
                        .zip(&b.points)
                        .all(|(p, q)| (q.x - p.x, q.y - p.y) == moved_by);
                !same_route
            }
            _ => was.w != is.w || was.h != is.h,
        };
        if changed || reshaped {
            elements.push(DiffElement {
                element: Element {
                    status: Status::Same,
                    ..old.clone()
                },
                role: Role::Old,
            });
            elements.push(DiffElement {
                element: Element {
                    status: if changed { Status::Changed } else { Status::Same },
                    ..element.clone()
                },
                role: Role::New,
            });
        } else if was.x != is.x || was.y != is.y {
            elements.push(DiffElement {
                element: element.clone(),
                role: Role::Moved {
                    dx: was.x - is.x,
                    dy: was.y - is.y,
                },
            });
        } else {
            elements.push(DiffElement {
                element: element.clone(),
                role: Role::Kept,
            });
        }
    }
    for element in &before.elements {
        if after.element(&element.id).is_none() {
            elements.push(DiffElement {
                element: Element {
                    status: Status::Removed,
                    ..element.clone()
                },
                role: Role::Removed,
            });
        }
    }
    // Regions behind lines, and lines behind boxes, as each layout draws
    // them.
    elements.sort_by_key(|item| match item.element.shape {
        Shape::Region(_) => 0,
        Shape::Line(_) => 1,
        _ => 2,
    });
    let mut scene = after.clone();
    scene.width = before.width.max(after.width);
    scene.height = before.height.max(after.height);
    Diff { scene, elements }
}
