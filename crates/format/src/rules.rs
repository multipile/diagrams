//! The rules a diagram is held to, whatever file it was read from.
//!
//! Some hold in every type: how long a title, a name or a label may be, how
//! many elements may be focal, and that every key a line names is declared.
//! The rest are a type's own limits, which keep a diagram to one idea. Over a
//! limit, a diagram is two diagrams.
//!
//! They are adapted from Diagram Design, an agent skill by Cathryn Lavery
//! (github.com/cathrynlavery/diagram-design), under the MIT licence, as the
//! drawing rules of multipile's diagrams.

use std::collections::{BTreeMap, BTreeSet};

use crate::finding::{Report, did_you_mean};
use crate::model::*;

pub const TITLE_MAX: usize = 60;
pub const NAME_MAX: usize = 24;
pub const SUB_MAX: usize = 32;
pub const LABEL_MAX: usize = 14;
pub const FOCAL_MAX: usize = 2;

pub(crate) fn check(diagram: &Diagram, report: &mut Report) {
    let mut rules = Rules {
        report,
        focal: Vec::new(),
    };
    rules.frame(&diagram.frame);
    match &diagram.body {
        Body::Architecture(body) => rules.architecture(body),
        Body::Sequence(body) => rules.sequence(body),
        Body::State(body) => rules.state(body),
        Body::Swimlane(body) => rules.swimlane(body),
        Body::Process(body) => rules.process(body),
        Body::Tree(body) => rules.tree(body),
        Body::Dependency(body) => rules.dependency(body),
        Body::Nested(body) => rules.nested(body),
        Body::Treemap(body) => rules.treemap(body),
    }
    rules.focal_total();
}

struct Rules<'r> {
    report: &'r mut Report,
    /// Where each focal element is.
    focal: Vec<Span>,
}

impl Rules<'_> {
    fn frame(&mut self, frame: &Frame) {
        self.length("title", &frame.title, TITLE_MAX, frame.title_span);
    }

    fn length(&mut self, what: &str, text: &str, max: usize, span: Span) {
        let count = text.chars().count();
        if count > max {
            self.report.help(
                "too-long",
                span,
                format!("{what} is {count} characters, and may be {max}"),
                format!("say it in fewer words: {text:?}"),
            );
        }
    }

    fn item(&mut self, item: &Item) {
        self.length("a name", &item.name, NAME_MAX, item.span);
        if let Some(sub) = &item.sub {
            self.length("a sub", sub, SUB_MAX, item.span);
        }
        if item.focal {
            self.focal.push(item.span);
        }
    }

    fn label(&mut self, label: &str, span: Span) {
        self.length("a label", label, LABEL_MAX, span);
    }

    fn line(&mut self, line: &Line) {
        if let Some(label) = &line.label {
            self.label(label, line.span);
        }
        if line.focal {
            self.focal.push(line.span);
        }
    }

    /// Each item's key is declared once.
    fn unique<'i>(&mut self, what: &str, items: impl IntoIterator<Item = (&'i str, Span)>) -> BTreeSet<&'i str> {
        let mut seen = BTreeSet::new();
        for (key, span) in items {
            if !seen.insert(key) {
                self.report.help(
                    "key-repeated",
                    span,
                    format!("there are two {what} with the key {key}"),
                    "give each its own key",
                );
            }
        }
        seen
    }

    /// `key` is one of `keys`.
    fn names(&mut self, what: &str, key: &str, keys: &BTreeSet<&str>, span: Span) -> bool {
        if keys.contains(key) {
            return true;
        }
        let help = did_you_mean(key, keys.iter().copied())
            .unwrap_or_else(|| format!("the {what} are {}", keys.iter().copied().collect::<Vec<_>>().join(", ")));
        self.report.help(
            "key-unknown",
            span,
            format!("there are no {what} with the key {key}"),
            help,
        );
        false
    }

    fn limit(&mut self, kind: Kind, what: &str, count: usize, max: usize, span: Span) {
        if count > max {
            self.report.help(
                "over-limit",
                span,
                format!(
                    "a {} diagram has at most {max} {what}, and this one has {count}",
                    kind.word()
                ),
                "draw two diagrams: an overview, and a detail",
            );
        }
    }

    fn focal_total(&mut self) {
        if self.focal.len() > FOCAL_MAX {
            self.report.help(
                "too-many-focal",
                self.focal[FOCAL_MAX],
                format!(
                    "{} elements are focal, and a diagram has at most {FOCAL_MAX}",
                    self.focal.len()
                ),
                "an accent on many elements says nothing: keep it on the one or two the diagram is about",
            );
        }
    }

    fn lines(&mut self, what: &str, lines: &[Line], keys: &BTreeSet<&str>) {
        for line in lines {
            self.names(what, &line.from, keys, line.span);
            self.names(what, &line.to, keys, line.span);
            self.line(line);
        }
    }

    fn architecture(&mut self, body: &Architecture) {
        let kind = Kind::Architecture;
        let all: Vec<&Item> = body
            .boxes
            .iter()
            .flat_map(|held| std::iter::once(&held.item).chain(&held.holds))
            .collect();
        let keys = self.unique("boxes", all.iter().map(|item| (item.key.as_str(), item.span)));
        for item in &all {
            self.item(item);
        }
        for held in &body.boxes {
            self.limit(kind, "boxes inside one box", held.holds.len(), 3, held.item.span);
        }
        self.limit(kind, "boxes", all.len(), 9, span_of(&all));
        self.limit(
            kind,
            "lines",
            body.lines.len(),
            12,
            body.lines.first().map_or_else(Span::default, |line| line.span),
        );
        let zones: Vec<&Tier> = body.tiers.iter().filter(|tier| tier.zone.is_some()).collect();
        if let Some(fourth) = zones.get(3) {
            self.limit(kind, "zones", zones.len(), 3, fourth.span);
        }
        let outer: BTreeSet<&str> = body.boxes.iter().map(|held| held.item.key.as_str()).collect();
        let inner: BTreeSet<&str> = body
            .boxes
            .iter()
            .flat_map(|held| held.holds.iter().map(|item| item.key.as_str()))
            .collect();
        let mut placed: BTreeMap<&str, Span> = BTreeMap::new();
        for tier in &body.tiers {
            for key in &tier.keys {
                if inner.contains(key.as_str()) {
                    self.report.help(
                        "held-in-tier",
                        tier.span,
                        format!("{key} is held by another box, and goes where its holder goes"),
                        "leave it out of the tiers",
                    );
                } else if self.names("boxes", key, &outer, tier.span) && placed.insert(key, tier.span).is_some() {
                    self.report.help(
                        "tier-repeated",
                        tier.span,
                        format!("{key} is in two tiers"),
                        "put each box in one tier",
                    );
                }
            }
        }
        for held in &body.boxes {
            if !placed.contains_key(held.item.key.as_str()) {
                self.report.help(
                    "tier-missing",
                    held.item.span,
                    format!("{} is in no tier", held.item.key),
                    "add it to the tier the flow reaches it in",
                );
            }
        }
        self.lines("boxes", &body.lines, &keys);
    }

    fn sequence(&mut self, body: &Sequence) {
        let kind = Kind::Sequence;
        let keys = self.unique("actors", body.actors.iter().map(|item| (item.key.as_str(), item.span)));
        for actor in &body.actors {
            self.item(actor);
        }
        self.limit(
            kind,
            "actors",
            body.actors.len(),
            5,
            span_of(&body.actors.iter().collect::<Vec<_>>()),
        );
        let mut arrows = Vec::new();
        let mut fragments = Vec::new();
        collect_messages(&body.messages, 0, &mut arrows, &mut fragments);
        for arrow in &arrows {
            self.names("actors", &arrow.from, &keys, arrow.span);
            self.names("actors", &arrow.to, &keys, arrow.span);
            self.label(&arrow.label, arrow.span);
            if arrow.focal {
                self.focal.push(arrow.span);
            }
        }
        if let Some(thirteenth) = arrows.get(12) {
            self.limit(kind, "messages", arrows.len(), 12, thirteenth.span);
        }
        for (fragment, depth) in &fragments {
            if *depth > 0 {
                self.report.help(
                    "fragment-nested",
                    fragment.span,
                    "an opt, loop or alt is not drawn inside another",
                    "draw the inner one as a diagram of its own",
                );
            }
        }
        let alts = fragments
            .iter()
            .filter(|(fragment, _)| fragment.kind == FragmentKind::Alt)
            .count();
        let allowed = if alts == 0 { 2 } else { 1 };
        if fragments.len() > allowed {
            self.report.help(
                "over-limit",
                fragments[allowed].0.span,
                format!(
                    "a sequence diagram has at most one opt, loop or alt (two when both are opt or loop), and this one has {}",
                    fragments.len()
                ),
                "draw two diagrams: the usual path, and the path that fails",
            );
        }
    }

    fn state(&mut self, body: &StateMachine) {
        let kind = Kind::State;
        let keys = self.unique(
            "states",
            body.states
                .iter()
                .map(|state| (state.item.key.as_str(), state.item.span)),
        );
        for state in &body.states {
            self.item(&state.item);
        }
        let states: Vec<&Item> = body.states.iter().map(|state| &state.item).collect();
        self.limit(kind, "states", states.len(), 9, span_of(&states));
        let starts: Vec<&State> = body.states.iter().filter(|state| state.start).collect();
        match starts.len() {
            1 => {}
            0 => self.report.help(
                "start-missing",
                span_of(&states),
                "a state diagram has one state it starts in",
                "add start=#true to that state",
            ),
            _ => self.report.help(
                "start-repeated",
                starts[1].item.span,
                "a state diagram starts in one state",
                "keep start=#true on one state",
            ),
        }
        for transition in &body.transitions {
            self.names("states", &transition.from, &keys, transition.span);
            self.names("states", &transition.to, &keys, transition.span);
            self.label(&transition.on, transition.span);
            for part in [&transition.guard, &transition.action].into_iter().flatten() {
                self.label(part, transition.span);
            }
            if transition.focal {
                self.focal.push(transition.span);
            }
        }
        if let Some(any) = &body.from_any {
            self.names("states", &any.to, &keys, any.span);
            self.label(&any.on, any.span);
        }
        if let Some(thirteenth) = body.transitions.get(12) {
            self.limit(kind, "transitions", body.transitions.len(), 12, thirteenth.span);
        } else if body.transitions.len() > 2 * body.states.len().max(1) {
            let span = body.transitions[2 * body.states.len()].span;
            self.report.help(
                "over-limit",
                span,
                "a state diagram has at most twice as many transitions as states",
                "a transition every state makes is one `any`, not a line from each",
            );
        }
    }

    fn swimlane(&mut self, body: &Swimlane) {
        let kind = Kind::Swimlane;
        self.unique("lanes", body.lanes.iter().map(|lane| (lane.key.as_str(), lane.span)));
        let steps: Vec<&Item> = body.lanes.iter().flat_map(|lane| &lane.steps).collect();
        let keys = self.unique("steps", steps.iter().map(|step| (step.key.as_str(), step.span)));
        for step in &steps {
            self.item(step);
        }
        for lane in &body.lanes {
            self.length("a lane's name", &lane.name, NAME_MAX, lane.span);
        }
        if let Some(sixth) = body.lanes.get(5) {
            self.limit(kind, "lanes", body.lanes.len(), 5, sixth.span);
        }
        self.limit(kind, "steps", steps.len(), 12, span_of(&steps));
        if let Some(thirteenth) = body.lines.get(12) {
            self.limit(kind, "lines", body.lines.len(), 12, thirteenth.span);
        }
        self.lines("steps", &body.lines, &keys);
    }

    fn process(&mut self, body: &Process) {
        let kind = Kind::Process;
        let columns = self.unique(
            "columns",
            body.columns.iter().map(|column| (column.key.as_str(), column.span)),
        );
        self.unique("lanes", body.lanes.iter().map(|lane| (lane.key.as_str(), lane.span)));
        let tasks: Vec<&Task> = body.lanes.iter().flat_map(|lane| &lane.tasks).collect();
        let keys = self.unique(
            "tasks",
            tasks.iter().map(|task| (task.item.key.as_str(), task.item.span)),
        );
        if let Some(seventh) = body.lanes.get(6) {
            self.limit(kind, "lanes", body.lanes.len(), 6, seventh.span);
        }
        if let Some(thirteenth) = body.columns.get(12) {
            self.limit(kind, "columns", body.columns.len(), 12, thirteenth.span);
        }
        let focal_columns: Vec<&Column> = body.columns.iter().filter(|column| column.focal).collect();
        if let Some(second) = focal_columns.get(1) {
            self.report.help(
                "too-many-focal",
                second.span,
                "a process diagram has at most one focal column",
                "keep focal=#true on the step the diagram is about",
            );
        }
        let focal_tasks: Vec<&&Task> = tasks.iter().filter(|task| task.item.focal).collect();
        if let Some(second) = focal_tasks.get(1) {
            self.report.help(
                "too-many-focal",
                second.item.span,
                "a process diagram has at most one focal task",
                "keep focal=#true on the task that receives the handoff that matters",
            );
        }
        for lane in &body.lanes {
            self.length("a lane's name", &lane.name, NAME_MAX, lane.span);
            if lane.badge.len() != 3 || !lane.badge.chars().all(|c| c.is_ascii_uppercase()) {
                self.report.help(
                    "badge-invalid",
                    lane.span,
                    format!("a badge is three capitals, not {:?}", lane.badge),
                    "such as badge=OPS",
                );
            }
            let mut cells = BTreeSet::new();
            for task in &lane.tasks {
                self.length("a name", &task.item.name, 32, task.item.span);
                if let Some(sub) = &task.item.sub {
                    self.length("a sub", sub, SUB_MAX, task.item.span);
                }
                if let Some(tool) = &task.tool {
                    self.length("a tool", tool, NAME_MAX, task.item.span);
                }
                if self.names("columns", &task.column, &columns, task.item.span) && !cells.insert(task.column.as_str())
                {
                    self.report.help(
                        "cell-taken",
                        task.item.span,
                        format!("lane {} has two tasks in column {}", lane.key, task.column),
                        "a cell holds one task: move one to a column of its own",
                    );
                }
            }
        }
        for line in &body.lines {
            self.names("tasks", &line.from, &keys, line.span);
            self.names("tasks", &line.to, &keys, line.span);
        }
    }

    fn tree(&mut self, body: &Tree) {
        let kind = Kind::Tree;
        let all = body.root.walk();
        self.unique(
            "items",
            all.iter().map(|branch| (branch.item.key.as_str(), branch.item.span)),
        );
        for branch in &all {
            self.item(&branch.item);
            if let Some(sixth) = branch.children.get(5) {
                self.limit(
                    kind,
                    "children of one parent",
                    branch.children.len(),
                    5,
                    sixth.item.span,
                );
            }
        }
        let depth = body.root.depth();
        if depth > 4 {
            let deepest = all
                .iter()
                .find(|branch| depth_of(&body.root, &branch.item.key) == Some(5));
            self.limit(
                kind,
                "levels, the root included",
                depth,
                4,
                deepest.map_or(body.root.item.span, |branch| branch.item.span),
            );
        }
        let focal: Vec<&&Branch> = all.iter().filter(|branch| branch.item.focal).collect();
        if let Some(second) = focal.get(1) {
            self.report.help(
                "too-many-focal",
                second.item.span,
                "the accent goes on the root or on one leaf, not both",
                "keep focal=#true on one item",
            );
        }
        for branch in focal {
            if !branch.children.is_empty() && branch.item.key != body.root.item.key {
                self.report.help(
                    "focal-not-root-or-leaf",
                    branch.item.span,
                    "in a tree, the accent goes on the root or on a leaf",
                    "move focal=#true to the root or to one of the items without children",
                );
            }
        }
    }

    fn dependency(&mut self, body: &Dependency) {
        let kind = Kind::Dependency;
        let keys = self.unique("boxes", body.boxes.iter().map(|item| (item.key.as_str(), item.span)));
        for item in &body.boxes {
            self.item(item);
        }
        let items: Vec<&Item> = body.boxes.iter().collect();
        self.limit(kind, "boxes", items.len(), 9, span_of(&items));
        if let Some(fifteenth) = body.lines.get(14) {
            self.limit(kind, "lines", body.lines.len(), 14, fifteenth.span);
        }
        self.lines("boxes", &body.lines, &keys);
        if let Some(cycle) = &body.cycle {
            self.names("boxes", &cycle.from, &keys, cycle.span);
            self.names("boxes", &cycle.to, &keys, cycle.span);
            if let Some(focal) = body.boxes.iter().find(|item| item.focal) {
                self.report.help(
                    "focal-with-cycle",
                    focal.span,
                    "the cycle is the accent, so no box is focal",
                    "take focal=#true off the box",
                );
            }
        }
        if body.boxes.iter().filter(|item| item.focal).count() > 1 {
            let second = body.boxes.iter().filter(|item| item.focal).nth(1).unwrap();
            self.report.help(
                "too-many-focal",
                second.span,
                "a dependency diagram has at most one focal box",
                "keep focal=#true on the box the diagram is about",
            );
        }
        match ranks(body) {
            Ok(ranks) => {
                let count = ranks.values().max().map_or(0, |rank| rank + 1);
                self.limit(kind, "ranks", count, 4, span_of(&items));
            }
            Err(span) => self.report.help(
                "cycle-unnamed",
                span,
                "the lines run in a circle",
                "name the one line that runs back as `cycle <from> <to>`, in place of `line`",
            ),
        }
    }

    fn nested(&mut self, body: &Nested) {
        let kind = Kind::Nested;
        for level in &body.levels {
            self.item(level);
        }
        let levels: Vec<&Item> = body.levels.iter().collect();
        if body.levels.len() < 3 {
            self.report.help(
                "under-limit",
                span_of(&levels),
                format!(
                    "a nested diagram has at least 3 levels, and this one has {}",
                    body.levels.len()
                ),
                "with fewer, say it in a sentence",
            );
        }
        if let Some(seventh) = body.levels.get(6) {
            self.limit(kind, "levels", body.levels.len(), 6, seventh.span);
        }
        if let Some(second) = body.levels.iter().filter(|level| level.focal).nth(1) {
            self.report.help(
                "too-many-focal",
                second.span,
                "a nested diagram has one focal level",
                "keep focal=#true on one level",
            );
        }
    }

    fn treemap(&mut self, body: &Treemap) {
        let kind = Kind::Treemap;
        self.unique(
            "parts",
            body.parts.iter().map(|part| (part.item.key.as_str(), part.item.span)),
        );
        let parts: Vec<&Item> = body.parts.iter().map(|part| &part.item).collect();
        for part in &body.parts {
            self.item(&part.item);
            if part.value.is_nan() || part.value <= 0.0 {
                self.report.help(
                    "value-not-positive",
                    part.item.span,
                    format!("a part's value is more than 0, not {}", part.value),
                    "a part with nothing in it is not a part",
                );
            }
            if let Some(short) = &part.short
                && (short.len() != 3 || !short.chars().all(|c| c.is_ascii_uppercase()))
            {
                self.report.help(
                    "short-invalid",
                    part.item.span,
                    format!("a short name is three capitals, not {short:?}"),
                    "such as short=CLI",
                );
            }
        }
        if body.parts.len() < 4 {
            self.report.help(
                "under-limit",
                span_of(&parts),
                format!("a treemap has at least 4 parts, and this one has {}", body.parts.len()),
                "with fewer, a sentence or a table says it better",
            );
        }
        if let Some(ninth) = body.parts.get(8) {
            self.limit(kind, "parts", body.parts.len(), 8, ninth.item.span);
        }
        if let Some(second) = body.parts.iter().filter(|part| part.item.focal).nth(1) {
            self.report.help(
                "too-many-focal",
                second.item.span,
                "a treemap has one focal part",
                "keep focal=#true on the part the diagram is about",
            );
        }
    }
}

fn span_of(items: &[&Item]) -> Span {
    items.last().map_or_else(Span::default, |item| item.span)
}

fn collect_messages<'m>(
    messages: &'m [Message],
    depth: usize,
    arrows: &mut Vec<&'m Arrow>,
    fragments: &mut Vec<(&'m Fragment, usize)>,
) {
    for message in messages {
        match message {
            Message::Arrow(arrow) => arrows.push(arrow),
            Message::Fragment(fragment) => {
                fragments.push((fragment, depth));
                for case in &fragment.cases {
                    collect_messages(&case.messages, depth + 1, arrows, fragments);
                }
            }
        }
    }
}

fn depth_of(branch: &Branch, key: &str) -> Option<usize> {
    if branch.item.key == key {
        return Some(1);
    }
    branch
        .children
        .iter()
        .find_map(|child| depth_of(child, key))
        .map(|depth| depth + 1)
}

/// Each box's rank in a dependency diagram: 0 for a box nothing depends on,
/// and one further along for each dependency. The cycle's line is left out.
/// When the lines run in a circle, the line that closes it.
pub fn ranks(body: &Dependency) -> Result<BTreeMap<String, usize>, Span> {
    let mut rank: BTreeMap<String, usize> = body.boxes.iter().map(|item| (item.key.clone(), 0)).collect();
    // Longest path by relaxing every line as many times as there are boxes:
    // a line still relaxing after that closes a circle.
    for _ in 0..=body.boxes.len() {
        let mut changed = None;
        for line in &body.lines {
            let (Some(&from), Some(&to)) = (rank.get(&line.from), rank.get(&line.to)) else {
                continue;
            };
            if to < from + 1 {
                rank.insert(line.to.clone(), from + 1);
                changed = Some(line.span);
            }
        }
        match changed {
            None => return Ok(rank),
            Some(span) if rank.values().any(|rank| *rank > body.boxes.len()) => return Err(span),
            Some(_) => {}
        }
    }
    Err(body.lines.last().map_or_else(Span::default, |line| line.span))
}
