//! Version 1 of the KDL format.
//!
//! Every type shares the frame (`title`, `desc`, `note`) and the way a box
//! is written:
//!
//! ```kdl
//! box <key> "<name>" pile="@cli" id="<id>" sub="<second line>" as=store focal=#true
//! ```
//!
//! The name may be left out: a box that stands for a pile shows the pile's
//! name, and any other box shows its key. Each type adds its own nodes, as
//! the functions below say.

use kdl::KdlNode;

use super::{Reader, Shape, entry_span, span};
use crate::finding::Report;
use crate::model::*;

const FRAME: [&str; 3] = ["title", "desc", "note"];
const ITEM_PROPS: [&str; 5] = ["pile", "id", "sub", "as", "focal"];

pub(super) fn read(node: &KdlNode, kind: Kind, report: &mut Report) -> Option<Diagram> {
    let mut r = Reader { report };
    r.shape(
        node,
        &Shape {
            usage: "diagram <type> version=1 { … }",
            args: (1, 1),
            props: &["version"],
            children: true,
        },
    );
    let mut frame = FrameParts::default();
    let children = Reader::children(node);
    let body = match kind {
        Kind::Architecture => architecture(&mut r, children, &mut frame).map(Body::Architecture),
        Kind::Sequence => sequence(&mut r, children, &mut frame).map(Body::Sequence),
        Kind::State => state(&mut r, children, &mut frame).map(Body::State),
        Kind::Swimlane => swimlane(&mut r, children, &mut frame).map(Body::Swimlane),
        Kind::Process => process(&mut r, children, &mut frame).map(Body::Process),
        Kind::Tree => tree(&mut r, children, &mut frame).map(Body::Tree),
        Kind::Dependency => dependency(&mut r, children, &mut frame).map(Body::Dependency),
        Kind::Nested => nested(&mut r, children, &mut frame).map(Body::Nested),
        Kind::Treemap => treemap(&mut r, children, &mut frame).map(Body::Treemap),
    };
    let frame = frame.finish(&mut r, node);
    Some(Diagram {
        frame: frame?,
        body: body?,
    })
}

#[derive(Default)]
struct FrameParts {
    title: Option<(String, Span)>,
    desc: Option<String>,
    note: Option<String>,
}

impl FrameParts {
    /// Reads `node` when it is part of the frame, and says whether it was.
    fn read(&mut self, r: &mut Reader, node: &KdlNode) -> bool {
        let name = node.name().value();
        let slot = match name {
            "title" | "desc" | "note" => name,
            _ => return false,
        };
        let usage = format!("{slot} \"<text>\"");
        if !r.shape(
            node,
            &Shape {
                usage: &usage,
                args: (1, 1),
                props: &[],
                children: false,
            },
        ) {
            return true;
        }
        let Some(text) = r.text_arg(node, 0, slot) else {
            return true;
        };
        let taken = match slot {
            "title" => self.title.replace((text, span(node))).is_some(),
            "desc" => self.desc.replace(text).is_some(),
            _ => self.note.replace(text).is_some(),
        };
        if taken {
            r.report
                .add("frame-repeated", span(node), format!("the diagram has one {slot}"));
        }
        true
    }

    fn finish(self, r: &mut Reader, diagram: &KdlNode) -> Option<Frame> {
        if self.title.is_none() {
            r.report.help(
                "title-missing",
                span(diagram),
                "the diagram has no title",
                "add: title \"<the subject, in at most 60 characters>\"",
            );
        }
        if self.desc.is_none() {
            r.report.help(
                "desc-missing",
                span(diagram),
                "the diagram has no desc",
                "add: desc \"<one sentence about what the diagram shows>\"",
            );
        }
        let (title, title_span) = self.title?;
        Some(Frame {
            title,
            desc: self.desc?,
            note: self.note,
            span: span(diagram),
            title_span,
        })
    }
}

/// How a box-like node is written, for [`item`].
struct ItemSpec<'s> {
    /// Whether its first argument is a key.
    keyed: bool,
    /// Properties it takes beyond a box's own.
    extra: &'s [&'s str],
    children: bool,
    usage: &'s str,
}

/// A box, an actor, a state, a step, a task, a part, a branch or a level.
/// A node with no key is given an empty one, for its caller to set.
fn item(r: &mut Reader, node: &KdlNode, spec: &ItemSpec) -> Option<Item> {
    let mut props: Vec<&str> = ITEM_PROPS.to_vec();
    props.extend_from_slice(spec.extra);
    let first = usize::from(spec.keyed);
    let fine = r.shape(
        node,
        &Shape {
            usage: spec.usage,
            args: (first, first + 1),
            props: &props,
            children: spec.children,
        },
    );
    let key = if spec.keyed {
        r.key_arg(node, 0).map(|(key, _)| key)
    } else {
        Some(String::new())
    };
    let name = if Reader::args(node).len() > first {
        r.text_arg(node, first, "a name")
    } else {
        None
    };
    let pile = pile(r, node);
    let sub = r.text_prop(node, "sub");
    let look = r
        .word_prop(node, "as", &Look::WORDS)
        .and_then(Look::from_word)
        .unwrap_or_default();
    let focal = r.bool_prop(node, "focal");
    let key = key?;
    let name = match (name, &pile) {
        (Some(name), _) => name,
        (None, Some(pile)) => pile.shown().to_string(),
        (None, None) if spec.keyed => key.clone(),
        (None, None) => {
            r.report.help(
                "name-missing",
                span(node),
                format!("`{}` has neither a name nor a pile", node.name().value()),
                format!("write it as: {}", spec.usage),
            );
            return None;
        }
    };
    if !fine {
        return None;
    }
    Some(Item {
        key,
        name,
        sub,
        pile,
        look,
        focal,
        span: span(node),
    })
}

fn pile(r: &mut Reader, node: &KdlNode) -> Option<Pile> {
    let entry = Reader::prop_entry(node, "pile")?;
    let reference = r.text(entry, "pile")?;
    if !(reference.starts_with('@') && reference.len() > 1 || reference.starts_with(":/")) {
        r.report.help(
            "pile-invalid",
            entry_span(entry),
            format!("{reference:?} names no pile"),
            "name a pile as pile names one: \"@cli\" by its name, or \":/crates/pile\" by its path",
        );
        return None;
    }
    let id = match Reader::prop_entry(node, "id") {
        Some(entry) => {
            let id = r.text(entry, "id")?;
            if id.len() != 32 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
                r.report.help(
                    "pile-id-invalid",
                    entry_span(entry),
                    format!("{id:?} is not a pile's id"),
                    "a pile's id is 32 hex characters, as `pile list --full` prints it",
                );
                return None;
            }
            Some(id)
        }
        None => None,
    };
    Some(Pile { reference, id })
}

fn flow(r: &mut Reader, node: &KdlNode) -> Option<Flow> {
    if !r.shape(
        node,
        &Shape {
            usage: "flow down  or  flow right",
            args: (1, 1),
            props: &[],
            children: false,
        },
    ) {
        return None;
    }
    let entry = Reader::args(node)[0];
    let word = r.text(entry, "the flow")?;
    match word.as_str() {
        "down" => Some(Flow::Down),
        "right" => Some(Flow::Right),
        _ => {
            r.report.help(
                "word-unknown",
                entry_span(entry),
                format!("the flow cannot be {word}"),
                "the flow is down or right",
            );
            None
        }
    }
}

/// Two keys, and the properties `props` allows.
fn ends(r: &mut Reader, node: &KdlNode, usage: &str, props: &[&str]) -> Option<(String, String)> {
    let fine = r.shape(
        node,
        &Shape {
            usage,
            args: (2, 2),
            props,
            children: false,
        },
    );
    let from = r.key_arg(node, 0);
    let to = r.key_arg(node, 1);
    if !fine {
        return None;
    }
    Some((from?.0, to?.0))
}

fn line(r: &mut Reader, node: &KdlNode) -> Option<Line> {
    let usage = "line <from> <to> label=\"<label>\" kind=call focal=#true";
    let ends = ends(r, node, usage, &["label", "kind", "focal"]);
    let label = r.text_prop(node, "label");
    let kind = r
        .word_prop(node, "kind", &LineKind::WORDS)
        .and_then(LineKind::from_word)
        .unwrap_or_default();
    let focal = r.bool_prop(node, "focal");
    let (from, to) = ends?;
    Some(Line {
        from,
        to,
        label,
        kind,
        focal,
        span: span(node),
    })
}

/// A dependency line: two keys and nothing else.
fn bare_line(r: &mut Reader, node: &KdlNode, word: &str) -> Option<Line> {
    let usage = format!("{word} <from> <to>");
    let (from, to) = ends(r, node, &usage, &[])?;
    Some(Line {
        from,
        to,
        label: None,
        kind: LineKind::Call,
        focal: false,
        span: span(node),
    })
}

/// Runs `each` on every child that is not part of the frame, and gives the
/// frame the rest.
fn each(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts, mut each: impl FnMut(&mut Reader, &KdlNode)) {
    for child in children {
        if !frame.read(r, child) {
            each(r, child);
        }
    }
}

fn known(words: &[&'static str]) -> Vec<&'static str> {
    FRAME.iter().copied().chain(words.iter().copied()).collect()
}

/// ```kdl
/// diagram dependency version=1 {
///   flow down
///   box cli pile="@cli"
///   box storage pile="@storage" focal=#true
///   line cli storage
///   cycle storage cli
/// }
/// ```
fn dependency(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts) -> Option<Dependency> {
    let words = known(&["flow", "box", "line", "cycle"]);
    let mut out = Dependency {
        flow: Flow::Down,
        boxes: Vec::new(),
        lines: Vec::new(),
        cycle: None,
    };
    let mut ok = true;
    each(r, children, frame, |r, node| match node.name().value() {
        "flow" => match flow(r, node) {
            Some(flow) => out.flow = flow,
            None => ok = false,
        },
        "box" => match item(r, node, &BOX) {
            Some(item) => out.boxes.push(item),
            None => ok = false,
        },
        "line" => match bare_line(r, node, "line") {
            Some(line) => out.lines.push(line),
            None => ok = false,
        },
        "cycle" => match bare_line(r, node, "cycle") {
            Some(line) if out.cycle.is_none() => out.cycle = Some(line),
            Some(line) => {
                r.report.help(
                    "cycle-repeated",
                    line.span,
                    "a dependency diagram shows at most one cycle",
                    "draw another diagram for the second one",
                );
                ok = false;
            }
            None => ok = false,
        },
        _ => {
            r.unknown(node, "diagram dependency", &words);
            ok = false;
        }
    });
    ok.then_some(out)
}

const BOX: ItemSpec = ItemSpec {
    keyed: true,
    extra: &[],
    children: false,
    usage: "box <key> \"<name>\" pile=\"@pile\" sub=\"<second line>\" as=plain focal=#true",
};

/// ```kdl
/// diagram architecture version=1 {
///   flow right
///   box storage pile="@storage" {
///     box git "git backend"
///   }
///   tier cli piles
///   tier storage zone="the binary" trust=#true
///   line cli storage label="read" kind=call
/// }
/// ```
fn architecture(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts) -> Option<Architecture> {
    let words = known(&["flow", "box", "tier", "line"]);
    let mut out = Architecture {
        flow: Flow::Right,
        boxes: Vec::new(),
        tiers: Vec::new(),
        lines: Vec::new(),
    };
    let mut flow_given = false;
    let mut ok = true;
    let holder = ItemSpec { children: true, ..BOX };
    each(r, children, frame, |r, node| match node.name().value() {
        "flow" => match flow(r, node) {
            Some(flow) => {
                out.flow = flow;
                flow_given = true;
            }
            None => ok = false,
        },
        "box" => {
            let Some(item) = item(r, node, &holder) else {
                ok = false;
                return;
            };
            let mut holds = Vec::new();
            for child in Reader::children(node) {
                if child.name().value() != "box" {
                    r.unknown(child, "box", &["box"]);
                    ok = false;
                } else if let Some(held) = super::v1::item(r, child, &BOX) {
                    holds.push(held);
                } else {
                    ok = false;
                }
            }
            out.boxes.push(ArchBox { item, holds });
        }
        "tier" => {
            let fine = r.shape(
                node,
                &Shape {
                    usage: "tier <key> <key>… zone=\"<label>\" trust=#true",
                    args: (1, usize::MAX),
                    props: &["zone", "trust"],
                    children: false,
                },
            );
            let mut keys = Vec::new();
            for index in 0..Reader::args(node).len() {
                match r.key_arg(node, index) {
                    Some((key, _)) => keys.push(key),
                    None => ok = false,
                }
            }
            let label = r.text_prop(node, "zone");
            let trust = r.bool_prop(node, "trust");
            if trust && label.is_none() {
                r.report.help(
                    "trust-without-zone",
                    span(node),
                    "only a zone is drawn as a trust boundary",
                    "give the tier a zone=\"<label>\"",
                );
                ok = false;
            }
            ok &= fine;
            out.tiers.push(Tier {
                keys,
                zone: label.map(|label| Zone { label, trust }),
                span: span(node),
            });
        }
        "line" => match line(r, node) {
            Some(line) => out.lines.push(line),
            None => ok = false,
        },
        _ => {
            r.unknown(node, "diagram architecture", &words);
            ok = false;
        }
    });
    if !flow_given && ok {
        r.report.help(
            "flow-missing",
            Span::default(),
            "an architecture diagram says which way its flow runs",
            "add: flow right  or  flow down",
        );
        ok = false;
    }
    ok.then_some(out)
}

/// ```kdl
/// diagram sequence version=1 {
///   actor you "you" as=input
///   actor cli pile="@cli"
///   call you cli "render"
///   alt {
///     case "has findings" { reply cli you "findings" }
///     case "else" { reply cli you "svg" focal=#true }
///   }
/// }
/// ```
fn sequence(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts) -> Option<Sequence> {
    let words = known(&["actor", "call", "reply", "notify", "self", "opt", "loop", "alt"]);
    let mut actors = Vec::new();
    let mut messages = Vec::new();
    let mut ok = true;
    let actor = ItemSpec {
        usage: "actor <key> \"<name>\" pile=\"@pile\" sub=\"<second line>\" as=plain",
        ..BOX
    };
    each(r, children, frame, |r, node| match node.name().value() {
        "actor" => match item(r, node, &actor) {
            Some(item) => actors.push(item),
            None => ok = false,
        },
        _ => match message(r, node, &words) {
            Some(message) => messages.push(message),
            None => ok = false,
        },
    });
    ok.then_some(Sequence { actors, messages })
}

const MESSAGES: [&str; 7] = ["call", "reply", "notify", "self", "opt", "loop", "alt"];

fn message(r: &mut Reader, node: &KdlNode, words: &[&str]) -> Option<Message> {
    let name = node.name().value();
    let arrow = |kind, usage: &str, props: &[&str], r: &mut Reader| -> Option<Message> {
        let fine = r.shape(
            node,
            &Shape {
                usage,
                args: (3, 3),
                props,
                children: false,
            },
        );
        let from = r.key_arg(node, 0);
        let to = r.key_arg(node, 1);
        let label = r.text_arg(node, 2, "the label");
        let link = r.bool_prop(node, "link");
        let focal = r.bool_prop(node, "focal");
        if !fine {
            return None;
        }
        Some(Message::Arrow(Arrow {
            kind,
            from: from?.0,
            to: to?.0,
            label: label?,
            link,
            focal,
            span: span(node),
        }))
    };
    match name {
        "call" => arrow(
            ArrowKind::Call,
            "call <from> <to> \"<label>\" link=#true focal=#true",
            &["link", "focal"],
            r,
        ),
        "reply" => arrow(
            ArrowKind::Reply,
            "reply <from> <to> \"<label>\" focal=#true",
            &["focal"],
            r,
        ),
        "notify" => arrow(
            ArrowKind::Notify,
            "notify <from> <to> \"<label>\" focal=#true",
            &["focal"],
            r,
        ),
        "self" => {
            let fine = r.shape(
                node,
                &Shape {
                    usage: "self <actor> \"<label>\" focal=#true",
                    args: (2, 2),
                    props: &["focal"],
                    children: false,
                },
            );
            let actor = r.key_arg(node, 0);
            let label = r.text_arg(node, 1, "the label");
            let focal = r.bool_prop(node, "focal");
            if !fine {
                return None;
            }
            let actor = actor?.0;
            Some(Message::Arrow(Arrow {
                kind: ArrowKind::SelfCall,
                from: actor.clone(),
                to: actor,
                label: label?,
                link: false,
                focal,
                span: span(node),
            }))
        }
        "opt" | "loop" => {
            let usage = format!("{name} \"<guard>\" {{ <messages> }}");
            let fine = r.shape(
                node,
                &Shape {
                    usage: &usage,
                    args: (1, 1),
                    props: &[],
                    children: true,
                },
            );
            let guard = r.text_arg(node, 0, "the guard");
            let messages = messages(r, node, &MESSAGES);
            if !fine {
                return None;
            }
            Some(Message::Fragment(Fragment {
                kind: if name == "opt" {
                    FragmentKind::Opt
                } else {
                    FragmentKind::Loop
                },
                cases: vec![Case {
                    guard: guard?,
                    messages: messages?,
                    span: span(node),
                }],
                span: span(node),
            }))
        }
        "alt" => {
            let usage = "alt { case \"<guard>\" { … } case \"else\" { … } }";
            let fine = r.shape(
                node,
                &Shape {
                    usage,
                    args: (0, 0),
                    props: &[],
                    children: true,
                },
            );
            let mut cases = Vec::new();
            let mut ok = fine;
            for child in Reader::children(node) {
                if child.name().value() != "case" {
                    r.unknown(child, "alt", &["case"]);
                    ok = false;
                    continue;
                }
                let fine = r.shape(
                    child,
                    &Shape {
                        usage: "case \"<guard>\" { <messages> }",
                        args: (1, 1),
                        props: &[],
                        children: true,
                    },
                );
                let guard = r.text_arg(child, 0, "the guard");
                let messages = messages(r, child, &MESSAGES);
                match (fine, guard, messages) {
                    (true, Some(guard), Some(messages)) => cases.push(Case {
                        guard,
                        messages,
                        span: span(child),
                    }),
                    _ => ok = false,
                }
            }
            if ok && cases.len() != 2 {
                r.report.help(
                    "alt-cases",
                    span(node),
                    format!("an alt holds two cases, not {}", cases.len()),
                    usage,
                );
                ok = false;
            }
            ok.then(|| {
                Message::Fragment(Fragment {
                    kind: FragmentKind::Alt,
                    cases,
                    span: span(node),
                })
            })
        }
        _ => {
            r.unknown(node, "diagram sequence", words);
            None
        }
    }
}

fn messages(r: &mut Reader, node: &KdlNode, words: &[&str]) -> Option<Vec<Message>> {
    let mut out = Vec::new();
    let mut ok = true;
    for child in Reader::children(node) {
        match message(r, child, words) {
            Some(message) => out.push(message),
            None => ok = false,
        }
    }
    ok.then_some(out)
}

/// ```kdl
/// diagram state version=1 {
///   flow right
///   state written "Written" start=#true
///   state deleted "Deleted" end=#true
///   transition written deleted on="git rm" guard="clean" action="log"
///   any deleted on="git rm"
/// }
/// ```
fn state(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts) -> Option<StateMachine> {
    let words = known(&["flow", "state", "transition", "any"]);
    let mut out = StateMachine {
        flow: Flow::Right,
        states: Vec::new(),
        transitions: Vec::new(),
        from_any: None,
    };
    let mut ok = true;
    let spec = ItemSpec {
        keyed: true,
        extra: &["start", "end"],
        children: false,
        usage: "state <key> \"<name>\" sub=\"<second line>\" start=#true end=#true focal=#true",
    };
    each(r, children, frame, |r, node| match node.name().value() {
        "flow" => match flow(r, node) {
            Some(flow) => out.flow = flow,
            None => ok = false,
        },
        "state" => {
            let start = r.bool_prop(node, "start");
            let end = r.bool_prop(node, "end");
            match item(r, node, &spec) {
                Some(item) => out.states.push(State { item, start, end }),
                None => ok = false,
            }
        }
        "transition" => {
            let usage = "transition <from> <to> on=\"<event>\" guard=\"<guard>\" action=\"<action>\" focal=#true";
            let ends = ends(r, node, usage, &["on", "guard", "action", "focal"]);
            let on = r.text_prop(node, "on");
            let guard = r.text_prop(node, "guard");
            let action = r.text_prop(node, "action");
            let focal = r.bool_prop(node, "focal");
            let Some((from, to)) = ends else {
                ok = false;
                return;
            };
            let Some(on) = on else {
                r.report.help(
                    "event-missing",
                    span(node),
                    "every transition has the event that causes it",
                    "add on=\"<event>\"",
                );
                ok = false;
                return;
            };
            out.transitions.push(Transition {
                from,
                to,
                on,
                guard,
                action,
                focal,
                span: span(node),
            });
        }
        "any" => {
            let fine = r.shape(
                node,
                &Shape {
                    usage: "any <to> on=\"<event>\"",
                    args: (1, 1),
                    props: &["on"],
                    children: false,
                },
            );
            let to = r.key_arg(node, 0);
            let on = r.text_prop(node, "on");
            match (fine, to, on) {
                (true, Some((to, _)), Some(on)) if out.from_any.is_none() => {
                    out.from_any = Some(FromAny {
                        to,
                        on,
                        span: span(node),
                    })
                }
                (true, Some(_), Some(_)) => {
                    r.report.add(
                        "any-repeated",
                        span(node),
                        "a state diagram has at most one transition from any state",
                    );
                    ok = false;
                }
                (true, Some(_), None) => {
                    r.report.help(
                        "event-missing",
                        span(node),
                        "every transition has the event that causes it",
                        "add on=\"<event>\"",
                    );
                    ok = false;
                }
                _ => ok = false,
            }
        }
        _ => {
            r.unknown(node, "diagram state", &words);
            ok = false;
        }
    });
    ok.then_some(out)
}

/// ```kdl
/// diagram swimlane version=1 {
///   lane author "Author" {
///     step read "Read the intent" sub="pile show"
///   }
///   line read edit label="check"
/// }
/// ```
fn swimlane(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts) -> Option<Swimlane> {
    let words = known(&["lane", "line"]);
    let mut out = Swimlane {
        lanes: Vec::new(),
        lines: Vec::new(),
    };
    let mut ok = true;
    let step = ItemSpec {
        usage: "step <key> \"<name>\" sub=\"<second line>\" focal=#true",
        ..BOX
    };
    each(r, children, frame, |r, node| match node.name().value() {
        "lane" => {
            let Some((key, name)) = lane_head(r, node, "lane <key> \"<name>\" { step … }", &[]) else {
                ok = false;
                return;
            };
            let mut steps = Vec::new();
            for child in Reader::children(node) {
                if child.name().value() != "step" {
                    r.unknown(child, "lane", &["step"]);
                    ok = false;
                } else if let Some(item) = item(r, child, &step) {
                    steps.push(item);
                } else {
                    ok = false;
                }
            }
            out.lanes.push(Lane {
                key,
                name,
                steps,
                span: span(node),
            });
        }
        "line" => match line(r, node) {
            Some(line) => out.lines.push(line),
            None => ok = false,
        },
        _ => {
            r.unknown(node, "diagram swimlane", &words);
            ok = false;
        }
    });
    ok.then_some(out)
}

/// A lane's key and name. Its name may be left out, and is then its key.
fn lane_head(r: &mut Reader, node: &KdlNode, usage: &str, props: &[&str]) -> Option<(String, String)> {
    let fine = r.shape(
        node,
        &Shape {
            usage,
            args: (1, 2),
            props,
            children: true,
        },
    );
    let key = r.key_arg(node, 0);
    let name = if Reader::args(node).len() > 1 {
        r.text_arg(node, 1, "a name")
    } else {
        key.as_ref().map(|(key, _)| key.clone())
    };
    if !fine {
        return None;
    }
    Some((key?.0, name?))
}

/// ```kdl
/// diagram process version=1 {
///   column write "Write"
///   column render "Render" focal=#true
///   lane author "Author" badge=AUT {
///     task script "Write the script" column=write tool=editor in=file out=file
///   }
///   line script svg trigger=#true
/// }
/// ```
fn process(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts) -> Option<Process> {
    let words = known(&["column", "lane", "line"]);
    let mut out = Process {
        columns: Vec::new(),
        lanes: Vec::new(),
        lines: Vec::new(),
    };
    let mut ok = true;
    let task = ItemSpec {
        keyed: true,
        extra: &["column", "tool", "in", "out"],
        children: false,
        usage: "task <key> \"<title>\" column=<column> sub=\"…\" tool=\"…\" in=file out=table focal=#true",
    };
    each(r, children, frame, |r, node| match node.name().value() {
        "column" => {
            let fine = r.shape(
                node,
                &Shape {
                    usage: "column <key> \"<label>\" focal=#true",
                    args: (1, 2),
                    props: &["focal"],
                    children: false,
                },
            );
            let key = r.key_arg(node, 0);
            let label = if Reader::args(node).len() > 1 {
                r.text_arg(node, 1, "a label")
            } else {
                key.as_ref().map(|(key, _)| key.clone())
            };
            let focal = r.bool_prop(node, "focal");
            match (fine, key, label) {
                (true, Some((key, _)), Some(label)) => out.columns.push(Column {
                    key,
                    label,
                    focal,
                    span: span(node),
                }),
                _ => ok = false,
            }
        }
        "lane" => {
            let usage = "lane <key> \"<name>\" badge=ABC { task … }";
            let head = lane_head(r, node, usage, &["badge"]);
            let badge = r.text_prop(node, "badge");
            let Some((key, name)) = head else {
                ok = false;
                return;
            };
            let Some(badge) = badge else {
                r.report.help(
                    "badge-missing",
                    span(node),
                    "a process lane has a badge, shown on each of its tasks",
                    "add badge=<three capitals>, such as badge=OPS",
                );
                ok = false;
                return;
            };
            let mut tasks = Vec::new();
            for child in Reader::children(node) {
                if child.name().value() != "task" {
                    r.unknown(child, "lane", &["task"]);
                    ok = false;
                    continue;
                }
                let column = r.text_prop(child, "column");
                let tool = r.text_prop(child, "tool");
                let input = r.word_prop(child, "in", &Payload::WORDS).and_then(Payload::from_word);
                let output = r.word_prop(child, "out", &Payload::WORDS).and_then(Payload::from_word);
                let Some(item) = item(r, child, &task) else {
                    ok = false;
                    continue;
                };
                let Some(column) = column else {
                    r.report.help(
                        "column-missing",
                        span(child),
                        "a task sits in a column",
                        "add column=<the column's key>",
                    );
                    ok = false;
                    continue;
                };
                tasks.push(Task {
                    item,
                    column,
                    tool,
                    input,
                    output,
                });
            }
            out.lanes.push(ProcessLane {
                key,
                name,
                badge,
                tasks,
                span: span(node),
            });
        }
        "line" => {
            let usage = "line <from> <to> trigger=#true";
            let ends = ends(r, node, usage, &["trigger"]);
            let trigger = r.bool_prop(node, "trigger");
            match ends {
                Some((from, to)) => out.lines.push(Handoff {
                    from,
                    to,
                    trigger,
                    span: span(node),
                }),
                None => ok = false,
            }
        }
        _ => {
            r.unknown(node, "diagram process", &words);
            ok = false;
        }
    });
    ok.then_some(out)
}

const BRANCH: ItemSpec = ItemSpec {
    keyed: false,
    extra: &[],
    children: true,
    usage: "item \"<name>\" pile=\"@pile\" sub=\"<second line>\" focal=#true { item … }",
};

/// ```kdl
/// diagram tree version=1 {
///   flow down
///   item ".pile/" {
///     item "config" sub="kind, version, id, name"
///     item "diagrams/" { item "crates.diagram" focal=#true }
///   }
/// }
/// ```
fn tree(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts) -> Option<Tree> {
    let words = known(&["flow", "item"]);
    let mut flow_ = Flow::Down;
    let mut roots = Vec::new();
    let mut ok = true;
    each(r, children, frame, |r, node| match node.name().value() {
        "flow" => match flow(r, node) {
            Some(flow) => flow_ = flow,
            None => ok = false,
        },
        "item" => match branch(r, node, "") {
            Some(branch) => roots.push((branch, span(node))),
            None => ok = false,
        },
        _ => {
            r.unknown(node, "diagram tree", &words);
            ok = false;
        }
    });
    if !ok {
        return None;
    }
    match roots.len() {
        1 => Some(Tree {
            flow: flow_,
            root: roots.remove(0).0,
        }),
        0 => {
            r.report.help(
                "root-missing",
                Span::default(),
                "a tree has a root",
                "add: item \"<the root>\" { item … }",
            );
            None
        }
        _ => {
            r.report.help(
                "roots-many",
                roots[1].1,
                "a tree has one root",
                "put the items under one root, or draw a tree for each",
            );
            None
        }
    }
}

fn branch(r: &mut Reader, node: &KdlNode, parent: &str) -> Option<Branch> {
    let mut item = item(r, node, &BRANCH)?;
    item.key = if parent.is_empty() {
        item.name.clone()
    } else {
        format!("{parent} / {}", item.name)
    };
    let mut children = Vec::new();
    let mut ok = true;
    for child in Reader::children(node) {
        if child.name().value() != "item" {
            r.unknown(child, "item", &["item"]);
            ok = false;
        } else if let Some(branch) = branch(r, child, &item.key) {
            children.push(branch);
        } else {
            ok = false;
        }
    }
    ok.then_some(Branch { item, children })
}

/// ```kdl
/// diagram nested version=1 {
///   level "the git repository" {
///     level pile="@multipile" sub=":/" {
///       level "renderer/" focal=#true
///     }
///   }
/// }
/// ```
fn nested(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts) -> Option<Nested> {
    let words = known(&["level"]);
    let mut levels = Vec::new();
    let mut ok = true;
    let spec = ItemSpec {
        usage: "level \"<label>\" pile=\"@pile\" sub=\"<second line>\" focal=#true { level … }",
        ..BRANCH
    };
    let mut outer = 0;
    each(r, children, frame, |r, node| match node.name().value() {
        "level" => {
            outer += 1;
            if outer > 1 {
                r.report.help(
                    "levels-side-by-side",
                    span(node),
                    "each level is inside the one before, so only one is outermost",
                    "put this level inside another, or draw a diagram for it",
                );
                ok = false;
                return;
            }
            let mut node = node;
            loop {
                let Some(mut level) = item(r, node, &spec) else {
                    ok = false;
                    return;
                };
                level.key = format!("level-{}", levels.len() + 1);
                levels.push(level);
                let inner = Reader::children(node);
                match inner {
                    [] => return,
                    [only] if only.name().value() == "level" => node = only,
                    [only] => {
                        r.unknown(only, "level", &["level"]);
                        ok = false;
                        return;
                    }
                    [_, second, ..] => {
                        r.report.help(
                            "levels-side-by-side",
                            span(second),
                            "a level holds one level, not several",
                            "a nested diagram is one line of scopes; draw a tree for a branching one",
                        );
                        ok = false;
                        return;
                    }
                }
            }
        }
        _ => {
            r.unknown(node, "diagram nested", &words);
            ok = false;
        }
    });
    ok.then_some(Nested { levels })
}

/// ```kdl
/// diagram treemap version=1 {
///   measure "lines of Rust"
///   source "wc -l, 2026-11-11"
///   part cli pile="@cli" value=7464 short=CLI
///   part render "render" value=6845 focal=#true
/// }
/// ```
fn treemap(r: &mut Reader, children: &[KdlNode], frame: &mut FrameParts) -> Option<Treemap> {
    let words = known(&["measure", "source", "part"]);
    let mut measure = None;
    let mut source = None;
    let mut parts = Vec::new();
    let mut ok = true;
    let spec = ItemSpec {
        keyed: true,
        extra: &["value", "short"],
        children: false,
        usage: "part <key> \"<name>\" pile=\"@pile\" value=<number> short=ABC focal=#true",
    };
    each(r, children, frame, |r, node| match node.name().value() {
        name @ ("measure" | "source") => {
            let usage = format!("{name} \"<text>\"");
            let fine = r.shape(
                node,
                &Shape {
                    usage: &usage,
                    args: (1, 1),
                    props: &[],
                    children: false,
                },
            );
            let text = r.text_arg(node, 0, name);
            match (fine, text) {
                (true, Some(text)) => {
                    if name == "measure" {
                        measure = Some(text);
                    } else {
                        source = Some(text);
                    }
                }
                _ => ok = false,
            }
        }
        "part" => {
            let value = r.number_prop(node, "value");
            let short = r.text_prop(node, "short");
            let Some(item) = item(r, node, &spec) else {
                ok = false;
                return;
            };
            let Some(value) = value else {
                r.report.help(
                    "value-missing",
                    span(node),
                    "a part has a value: how much of the whole it is",
                    "add value=<a number more than 0>",
                );
                ok = false;
                return;
            };
            parts.push(Part { item, value, short });
        }
        _ => {
            r.unknown(node, "diagram treemap", &words);
            ok = false;
        }
    });
    if ok {
        for (what, given) in [("measure", &measure), ("source", &source)] {
            if given.is_none() {
                r.report.help(
                    "treemap-frame-missing",
                    Span::default(),
                    format!("a treemap says its {what}"),
                    format!("add: {what} \"<text>\""),
                );
                ok = false;
            }
        }
    }
    ok.then(|| Treemap {
        measure: measure.unwrap_or_default(),
        source: source.unwrap_or_default(),
        parts,
    })
}
