//! What a diagram is, whatever file it was read from.
//!
//! Every version of every format is read into these types, so the html
//! builder knows one diagram and nothing of the files. A span points back
//! into the file an element came from, so a rule broken by the model can be
//! reported at the line that wrote it.

/// Where an element was written: a byte offset into the file, and a length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub offset: usize,
    pub len: usize,
}

/// One diagram: what every diagram has, and what its type has.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    pub frame: Frame,
    pub body: Body,
}

impl Diagram {
    pub fn kind(&self) -> Kind {
        self.body.kind()
    }
}

/// What every diagram has, whatever its type.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// The subject, drawn as the heading.
    pub title: String,
    /// One sentence about what the diagram shows, for a screen reader.
    pub desc: String,
    /// One sentence drawn under the diagram.
    pub note: Option<String>,
    /// The `diagram` node.
    pub span: Span,
    pub title_span: Span,
}

/// The type of a diagram, which says which shape of picture it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Architecture,
    Sequence,
    State,
    Swimlane,
    Process,
    Tree,
    Dependency,
    Nested,
    Treemap,
}

impl Kind {
    pub const ALL: [Kind; 9] = [
        Kind::Architecture,
        Kind::Sequence,
        Kind::State,
        Kind::Swimlane,
        Kind::Process,
        Kind::Tree,
        Kind::Dependency,
        Kind::Nested,
        Kind::Treemap,
    ];

    /// The word a file names the type by.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Architecture => "architecture",
            Kind::Sequence => "sequence",
            Kind::State => "state",
            Kind::Swimlane => "swimlane",
            Kind::Process => "process",
            Kind::Tree => "tree",
            Kind::Dependency => "dependency",
            Kind::Nested => "nested",
            Kind::Treemap => "treemap",
        }
    }

    pub fn from_word(word: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.word() == word)
    }
}

/// The direction a diagram's main flow runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flow {
    #[default]
    Down,
    Right,
}

/// What a box stands for, which sets its look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
    #[default]
    Plain,
    Store,
    External,
    Input,
    Optional,
    Security,
}

impl Look {
    pub const WORDS: [&'static str; 6] = ["plain", "store", "external", "input", "optional", "security"];

    pub fn word(self) -> &'static str {
        match self {
            Look::Plain => "plain",
            Look::Store => "store",
            Look::External => "external",
            Look::Input => "input",
            Look::Optional => "optional",
            Look::Security => "security",
        }
    }

    pub fn from_word(word: &str) -> Option<Look> {
        [
            Look::Plain,
            Look::Store,
            Look::External,
            Look::Input,
            Look::Optional,
            Look::Security,
        ]
        .into_iter()
        .find(|look| look.word() == word)
    }
}

/// A pile a box stands for: by its name, `@cli`, or by its `:/` path when it
/// has none, and by its id when the file gives one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pile {
    pub reference: String,
    pub id: Option<String>,
}

impl Pile {
    /// What a box shows for the pile: its name without the `@`, or its path.
    pub fn shown(&self) -> &str {
        self.reference.strip_prefix('@').unwrap_or(&self.reference)
    }
}

/// One thing a diagram shows: a box, an actor, a state, a step, a part.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// What other elements name it by. Unique among its kind in a diagram.
    pub key: String,
    /// What it shows.
    pub name: String,
    /// A second line, in monospace.
    pub sub: Option<String>,
    pub pile: Option<Pile>,
    pub look: Look,
    pub focal: bool,
    pub span: Span,
}

/// What a line stands for, which sets how it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineKind {
    /// One part uses another. A solid line.
    #[default]
    Call,
    /// An HTTP or API call, or a call outside the system.
    Link,
    /// A message nobody waits for. A dashed line.
    Async,
    /// A use that does not always happen. A dashed line.
    Optional,
}

impl LineKind {
    pub const WORDS: [&'static str; 4] = ["call", "link", "async", "optional"];

    pub fn from_word(word: &str) -> Option<LineKind> {
        match word {
            "call" => Some(LineKind::Call),
            "link" => Some(LineKind::Link),
            "async" => Some(LineKind::Async),
            "optional" => Some(LineKind::Optional),
            _ => None,
        }
    }
}

/// A line from one element to another, named by their keys. The arrowhead is
/// at `to`.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub kind: LineKind,
    pub focal: bool,
    pub span: Span,
}

/// What a diagram of each type holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Architecture(Architecture),
    Sequence(Sequence),
    State(StateMachine),
    Swimlane(Swimlane),
    Process(Process),
    Tree(Tree),
    Dependency(Dependency),
    Nested(Nested),
    Treemap(Treemap),
}

impl Body {
    pub fn kind(&self) -> Kind {
        match self {
            Body::Architecture(_) => Kind::Architecture,
            Body::Sequence(_) => Kind::Sequence,
            Body::State(_) => Kind::State,
            Body::Swimlane(_) => Kind::Swimlane,
            Body::Process(_) => Kind::Process,
            Body::Tree(_) => Kind::Tree,
            Body::Dependency(_) => Kind::Dependency,
            Body::Nested(_) => Kind::Nested,
            Body::Treemap(_) => Kind::Treemap,
        }
    }
}

/// The parts of a system, and what talks to what, in tiers along the flow.
#[derive(Debug, Clone, PartialEq)]
pub struct Architecture {
    pub flow: Flow,
    /// The boxes no other box holds, in the order they were written.
    pub boxes: Vec<ArchBox>,
    pub tiers: Vec<Tier>,
    pub lines: Vec<Line>,
}

/// A box, and the boxes drawn inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchBox {
    pub item: Item,
    pub holds: Vec<Item>,
}

/// The boxes of one tier, in order, and the zone drawn around them.
#[derive(Debug, Clone, PartialEq)]
pub struct Tier {
    pub keys: Vec<String>,
    pub zone: Option<Zone>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub label: String,
    /// Drawn dashed, as a trust boundary.
    pub trust: bool,
}

/// Messages between actors, in time order.
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    pub actors: Vec<Item>,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Arrow(Arrow),
    Fragment(Fragment),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowKind {
    /// A request that expects a reply.
    Call,
    /// The reply to a call.
    Reply,
    /// A message nobody waits for.
    Notify,
    /// A call an actor makes to itself.
    SelfCall,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arrow {
    pub kind: ArrowKind,
    pub from: String,
    /// The same as `from` for a call to itself.
    pub to: String,
    pub label: String,
    /// Drawn in the link colour, for an HTTP or API call.
    pub link: bool,
    pub focal: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentKind {
    /// Messages that happen only when the guard holds.
    Opt,
    /// Messages that repeat.
    Loop,
    /// One of two runs of messages.
    Alt,
}

/// Messages framed together: one case for `opt` and `loop`, two for `alt`.
#[derive(Debug, Clone, PartialEq)]
pub struct Fragment {
    pub kind: FragmentKind,
    pub cases: Vec<Case>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub guard: String,
    pub messages: Vec<Message>,
    pub span: Span,
}

/// States, and the events that move between them.
#[derive(Debug, Clone, PartialEq)]
pub struct StateMachine {
    pub flow: Flow,
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
    /// A transition every state can make, drawn as one note.
    pub from_any: Option<FromAny>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub item: Item,
    pub start: bool,
    pub end: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub from: String,
    pub to: String,
    /// The event that causes it.
    pub on: String,
    pub guard: Option<String>,
    pub action: Option<String>,
    pub focal: bool,
    pub span: Span,
}

impl Transition {
    /// `on [guard] / action`, as it is drawn.
    pub fn label(&self) -> String {
        let mut label = self.on.clone();
        if let Some(guard) = &self.guard {
            label.push_str(&format!(" [{guard}]"));
        }
        if let Some(action) = &self.action {
            label.push_str(&format!(" / {action}"));
        }
        label
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FromAny {
    pub to: String,
    pub on: String,
    pub span: Span,
}

/// The steps of one process, each in the lane of whoever does it.
#[derive(Debug, Clone, PartialEq)]
pub struct Swimlane {
    pub lanes: Vec<Lane>,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    pub key: String,
    pub name: String,
    pub steps: Vec<Item>,
    pub span: Span,
}

/// A process run by several teams: who does what, at which step, with which
/// tool, and what data goes in and out.
#[derive(Debug, Clone, PartialEq)]
pub struct Process {
    pub columns: Vec<Column>,
    pub lanes: Vec<ProcessLane>,
    pub lines: Vec<Handoff>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub key: String,
    pub label: String,
    pub focal: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessLane {
    pub key: String,
    pub name: String,
    /// Three capitals, shown on each of the lane's tasks.
    pub badge: String,
    pub tasks: Vec<Task>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// The task's key, title (as its name), sub and focus.
    pub item: Item,
    /// The key of the column it sits in.
    pub column: String,
    pub tool: Option<String>,
    pub input: Option<Payload>,
    pub output: Option<Payload>,
}

/// The kind of data that enters or leaves a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payload {
    List,
    Dataset,
    Table,
    File,
    Web,
}

impl Payload {
    pub const WORDS: [&'static str; 5] = ["list", "dataset", "table", "file", "web"];

    pub fn from_word(word: &str) -> Option<Payload> {
        match word {
            "list" => Some(Payload::List),
            "dataset" => Some(Payload::Dataset),
            "table" => Some(Payload::Table),
            "file" => Some(Payload::File),
            "web" => Some(Payload::Web),
            _ => None,
        }
    }

    /// The two capitals drawn on the tag.
    pub fn code(self) -> &'static str {
        match self {
            Payload::List => "LS",
            Payload::Dataset => "DB",
            Payload::Table => "TB",
            Payload::File => "FL",
            Payload::Web => "WB",
        }
    }
}

/// A line from one task to another. A trigger starts the next task, and
/// passes no data.
#[derive(Debug, Clone, PartialEq)]
pub struct Handoff {
    pub from: String,
    pub to: String,
    pub trigger: bool,
    pub span: Span,
}

/// A parent and its children, where each child has one parent.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub flow: Flow,
    pub root: Branch,
}

/// A box and the boxes below it. A branch's key is the names on its path
/// from the root, joined by ` / `, since nothing names a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub item: Item,
    pub children: Vec<Branch>,
}

impl Branch {
    /// The number of levels from this branch down, itself included.
    pub fn depth(&self) -> usize {
        1 + self.children.iter().map(Branch::depth).max().unwrap_or(0)
    }

    /// Every branch, this one first, depth first.
    pub fn walk(&self) -> Vec<&Branch> {
        let mut all = vec![self];
        for child in &self.children {
            all.extend(child.walk());
        }
        all
    }
}

/// What depends on what. A line points from a box to a box it depends on.
#[derive(Debug, Clone, PartialEq)]
pub struct Dependency {
    pub flow: Flow,
    pub boxes: Vec<Item>,
    pub lines: Vec<Line>,
    /// The one line that runs back.
    pub cycle: Option<Line>,
}

/// Scopes inside scopes, the outermost first.
#[derive(Debug, Clone, PartialEq)]
pub struct Nested {
    pub levels: Vec<Item>,
}

/// How one whole splits into parts, by size.
#[derive(Debug, Clone, PartialEq)]
pub struct Treemap {
    /// What the area stands for.
    pub measure: String,
    /// Where the numbers come from, and when.
    pub source: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub item: Item,
    pub value: f64,
    /// Three capitals, shown when the cell is too small for the name.
    pub short: Option<String>,
}
