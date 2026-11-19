//! Two versions of a diagram, element by element, in two steps.
//!
//! [`compare`] says what is drawn differently, as a [`Report`]: plain data
//! its caller reads, and may change. [`diff`] takes the two versions and a
//! report, and gives every element its place in the drawing of exactly
//! what the report says. Nothing between the two decides what changed: that
//! is the caller's, who may pass the report on as it is.
//!
//! Elements are matched by id, which a layout gives from what the diagram
//! names: `box:cli` is the box with the key `cli` in both versions. To
//! [`compare`], an element in the second version only was added, and one in
//! the first only was removed. An element in both is drawn differently when
//! what it draws differs, its signature: its name, its look, its label.
//! Where it is drawn is no difference: an element that only moved is drawn
//! moving, and is not reported. Neither is its metadata, which nothing here
//! reads.

use crate::scene::*;

/// What a [`Report`] names: an element or a line of a diagram.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Subject {
    /// An element the file names by a key: a box, an actor, a state, a
    /// step, a task, a part. A tree's item is named by the names on its
    /// path from the root, joined by ` / `, and a nested diagram's level by
    /// `level-1` for the outermost, `level-2` for the next.
    Element(String),
    /// A `line` or a `cycle`, by the keys of the two elements it joins.
    Line { from: String, to: String },
    /// Anything else a diagram draws, by the id its layout gives it, the
    /// same in every version: a lane `lane:<key>`, a message
    /// `message:<from>-><to>:<label>`, a transition
    /// `transition:<from>-><to>:<on>`, a zone `zone:<n>`. It is the id of
    /// the element's node in the document. An element or a line with a key
    /// can be named this way too: of two lines between the same two
    /// elements, `line:a->b` is the first and `line:a->b#2` the second.
    Shape(String),
}

impl Subject {
    pub fn element(key: impl Into<String>) -> Subject {
        Subject::Element(key.into())
    }

    pub fn line(from: impl Into<String>, to: impl Into<String>) -> Subject {
        Subject::Line {
            from: from.into(),
            to: to.into(),
        }
    }

    pub fn shape(id: impl Into<String>) -> Subject {
        Subject::Shape(id.into())
    }
}

/// What changed between two versions of a diagram, as its caller reports
/// it: plain data, to read and to change. [`compare`] gives the report of
/// what is drawn differently, and [`diff`] draws whatever a report says.
///
/// A subject in more than one list is drawn as the first of added, removed
/// and changed that names it. A subject neither version has is not drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub added: Vec<Subject>,
    pub removed: Vec<Subject>,
    pub changed: Vec<Subject>,
}

impl Report {
    /// Whether the report says nothing.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// Whether any list names `subject`.
    pub fn names(&self, subject: &Subject) -> bool {
        [&self.added, &self.removed, &self.changed]
            .iter()
            .any(|list| list.contains(subject))
    }

    /// Reports `subject` as changed, unless the report names it already.
    pub fn change(&mut self, subject: Subject) {
        if !self.names(&subject) {
            self.changed.push(subject);
        }
    }

    /// What the report says of an element: by its key or its two ends, or
    /// by its id.
    fn status(&self, element: &Element) -> Status {
        let keyed = element.shape.subject();
        let shape = Subject::Shape(element.id.clone());
        let named = |list: &[Subject]| {
            list.iter()
                .any(|subject| Some(subject) == keyed.as_ref() || *subject == shape)
        };
        if named(&self.added) {
            Status::Added
        } else if named(&self.removed) {
            Status::Removed
        } else if named(&self.changed) {
            Status::Changed
        } else {
            Status::Same
        }
    }
}

/// What is drawn differently between two scenes: each element the second
/// has alone as added, each the first has alone as removed, and each both
/// have whose signatures differ as changed.
///
/// An element is reported by its key and a line by its two ends, where the
/// file gives them and they name one element of each scene. Anything else
/// is reported by its id.
pub fn compare(before: &Scene, after: &Scene) -> Report {
    let subject = |element: &Element| match element.shape.subject() {
        Some(subject) if [before, after].iter().all(|scene| scene.count(&subject) <= 1) => subject,
        _ => Subject::Shape(element.id.clone()),
    };
    let mut report = Report::default();
    for element in &after.elements {
        match before.element(&element.id) {
            None => report.added.push(subject(element)),
            Some(old) if old.shape.signature() != element.shape.signature() => report.changed.push(subject(element)),
            Some(_) => {}
        }
    }
    for element in &before.elements {
        if after.element(&element.id).is_none() {
            report.removed.push(subject(element));
        }
    }
    report
}

/// What an element does as one version becomes the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The same, in the same place.
    Kept,
    /// The same, elsewhere: it slides from `dx, dy` away to where it is now.
    Moved { dx: i32, dy: i32 },
    /// In the second version only: it appears.
    Added,
    /// In the first version only: it fades.
    Removed,
    /// The first version of an element the two versions draw differently,
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

/// The two scenes as one diff, which shows exactly what `report` says. An
/// element is marked as the report names it, and one the report does not
/// name is not marked, though it is drawn differently in the two scenes.
/// Only how each element gets from the first scene to the second is worked
/// out here, from where and how each scene draws it.
pub fn diff(before: &Scene, after: &Scene, report: &Report) -> Diff {
    let mut elements = Vec::new();
    for element in &after.elements {
        let status = report.status(element);
        let Some(old) = before.element(&element.id) else {
            elements.push(DiffElement {
                element: Element {
                    status,
                    ..element.clone()
                },
                role: Role::Added,
            });
            continue;
        };
        let (was, is) = (old.shape.bounds(), element.shape.bounds());
        let redrawn = old.shape.signature() != element.shape.signature();
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
        if redrawn || reshaped {
            elements.push(DiffElement {
                element: Element {
                    status: Status::Same,
                    ..old.clone()
                },
                role: Role::Old,
            });
            elements.push(DiffElement {
                element: Element {
                    status,
                    ..element.clone()
                },
                role: Role::New,
            });
        } else {
            let role = if was.x != is.x || was.y != is.y {
                Role::Moved {
                    dx: was.x - is.x,
                    dy: was.y - is.y,
                }
            } else {
                Role::Kept
            };
            elements.push(DiffElement {
                element: Element {
                    status,
                    ..element.clone()
                },
                role,
            });
        }
    }
    for element in &before.elements {
        if after.element(&element.id).is_none() {
            elements.push(DiffElement {
                element: Element {
                    status: report.status(element),
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
