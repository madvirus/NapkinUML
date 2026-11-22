//! Project, diagram, shape and connector data. Saved as JSON.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DiagramKind {
    Class,
    Component,
    Deployment,
    UseCase,
    Sequence,
    StateMachine,
    Activity,
    Object,
    Package,
    Erd,
    Network,
    MindMap,
}

impl DiagramKind {
    pub const ALL: [DiagramKind; 12] = [
        DiagramKind::Class,
        DiagramKind::Component,
        DiagramKind::Deployment,
        DiagramKind::UseCase,
        DiagramKind::Sequence,
        DiagramKind::StateMachine,
        DiagramKind::Activity,
        DiagramKind::Object,
        DiagramKind::Package,
        DiagramKind::Erd,
        DiagramKind::Network,
        DiagramKind::MindMap,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DiagramKind::Class => "Class Diagram",
            DiagramKind::Component => "Component Diagram",
            DiagramKind::Deployment => "Deployment Diagram",
            DiagramKind::UseCase => "Use Case Diagram",
            DiagramKind::Sequence => "Sequence Diagram",
            DiagramKind::StateMachine => "State Machine Diagram",
            DiagramKind::Activity => "Activity Diagram",
            DiagramKind::Object => "Object Diagram",
            DiagramKind::Package => "Package Diagram",
            DiagramKind::Erd => "Entity Relationship Diagram",
            DiagramKind::Network => "Network Diagram",
            DiagramKind::MindMap => "Mind Map Diagram",
        }
    }

    /// Short description shown in the New Diagram dialog.
    pub fn description(self) -> &'static str {
        match self {
            DiagramKind::Class => "Shows the classes of a system, their attributes, operations and the relationships between them.",
            DiagramKind::Component => "Shows how components are wired together to form larger components or software systems.",
            DiagramKind::Deployment => "Shows the hardware nodes and the software artifacts deployed on them.",
            DiagramKind::UseCase => "Shows the actors of a system and the use cases they take part in.",
            DiagramKind::Sequence => "Shows how objects interact over time by exchanging messages along lifelines.",
            DiagramKind::StateMachine => "Shows the states of an object and the transitions between them.",
            DiagramKind::Activity => "Shows a flow of actions, decisions and parallel paths, optionally in swimlanes.",
            DiagramKind::Object => "Shows instances of classes and the links between them at one moment.",
            DiagramKind::Package => "Shows packages and the dependencies, imports and merges between them.",
            DiagramKind::Erd => "Shows database tables, their columns and the relationships between them.",
            DiagramKind::Network => "Shows network devices (routers, switches, firewalls, servers, PCs, phones...), the network zones they sit in and how they are connected.",
            DiagramKind::MindMap => "Shows one central topic with the ideas that branch out of it, and their sub-ideas. Select a topic and press Tab to add a sub-topic, Enter to add a sibling topic. The central topic picks the structure (mind map, logic chart or org chart); topics are placed automatically, and a dragged topic joins the topic it is dropped next to. The small circle beside a topic (or Ctrl+/) folds and unfolds its sub-topics.",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ShapeKind {
    // structural
    Class,
    Interface,
    Enumeration,
    Package,
    Subsystem,
    Note,
    /// Free text shown on the diagram: no border, line breaks kept (all diagrams).
    Text,
    Constraint,
    Collaboration,
    // component / deployment
    Component,
    Port,
    ProvidedInterface,
    RequiredInterface,
    Node,
    Device,
    ExecutionEnvironment,
    Artifact,
    DeploymentSpecification,
    Instance,
    // use case
    Actor,
    UseCase,
    System,
    // sequence
    Lifeline,
    ActorLifeline,
    AltFragment,
    LoopFragment,
    // state machine
    State,
    Initial,
    Final,
    Choice,
    Junction,
    Fork,
    Join,
    ShallowHistory,
    DeepHistory,
    EntryPoint,
    ExitPoint,
    Terminate,
    // activity
    Action,
    ActivityFinal,
    FlowFinal,
    Decision,
    Merge,
    ObjectNode,
    SendSignal,
    AcceptEvent,
    AcceptTimeEvent,
    VerticalSwimlane,
    HorizontalSwimlane,
    // ERD
    Entity,
    DbView,
    // network
    Router,
    Switch,
    Hub,
    Server,
    Firewall,
    WebFirewall,
    LoadBalancer,
    Pc,
    Laptop,
    Phone,
    AccessPoint,
    Storage,
    Database,
    Person,
    Cloud,
    NetworkZone,
    /// The idea a mind map grows from (drawn as a pill).
    CentralTopic,
    /// An idea branching off the central topic or another topic.
    Topic,
}

impl ShapeKind {
    /// Default size of a new shape (diagram units = pixels at 100%).
    pub fn default_size(self) -> (f32, f32) {
        use ShapeKind::*;
        match self {
            Class | Interface | Enumeration => (130.0, 80.0),
            Package | Subsystem => (160.0, 110.0),
            Note => (150.0, 60.0),
            Text => (120.0, 30.0),
            Constraint => (140.0, 40.0),
            Collaboration => (130.0, 60.0),
            Component => (150.0, 70.0),
            Port => (16.0, 16.0),
            ProvidedInterface => (24.0, 24.0),
            RequiredInterface => (36.0, 36.0),
            Node | Device | ExecutionEnvironment => (190.0, 130.0),
            Artifact | DeploymentSpecification => (140.0, 60.0),
            Instance => (140.0, 60.0),
            Actor => (40.0, 80.0),
            UseCase => (140.0, 60.0),
            System => (320.0, 380.0),
            Lifeline | ActorLifeline => (110.0, 420.0),
            AltFragment | LoopFragment => (320.0, 160.0),
            State => (120.0, 60.0),
            Initial => (24.0, 24.0),
            Final | ActivityFinal => (26.0, 26.0),
            FlowFinal => (24.0, 24.0),
            Choice | Decision | Merge => (32.0, 32.0),
            Junction => (16.0, 16.0),
            Fork | Join => (110.0, 8.0),
            ShallowHistory | DeepHistory => (26.0, 26.0),
            EntryPoint | ExitPoint => (20.0, 20.0),
            Terminate => (22.0, 22.0),
            Action => (120.0, 50.0),
            ObjectNode => (120.0, 46.0),
            SendSignal | AcceptEvent => (130.0, 46.0),
            AcceptTimeEvent => (28.0, 34.0),
            VerticalSwimlane => (440.0, 420.0),
            HorizontalSwimlane => (520.0, 320.0),
            Entity | DbView => (170.0, 90.0),
            Router => (60.0, 44.0),
            Switch => (70.0, 34.0),
            Hub => (64.0, 30.0),
            Server => (44.0, 64.0),
            Firewall | WebFirewall => (60.0, 48.0),
            LoadBalancer => (64.0, 40.0),
            Pc => (56.0, 50.0),
            Laptop => (60.0, 44.0),
            Phone => (30.0, 52.0),
            AccessPoint => (50.0, 40.0),
            Database => (50.0, 58.0),
            Person => (40.0, 50.0),
            Storage => (50.0, 56.0),
            Cloud => (130.0, 70.0),
            NetworkZone => (320.0, 200.0),
            CentralTopic => (170.0, 64.0),
            Topic => (120.0, 36.0),
        }
    }

    /// Shapes that hold other shapes: drawn first and they carry their contents when moved.
    pub fn is_container(self) -> bool {
        use ShapeKind::*;
        matches!(
            self,
            Package
                | Subsystem
                | Component
                | Instance
                | System
                | Node
                | Device
                | ExecutionEnvironment
                | AltFragment
                | LoopFragment
                | VerticalSwimlane
                | HorizontalSwimlane
                | NetworkZone
        )
    }

    pub fn is_swimlane(self) -> bool {
        matches!(self, ShapeKind::VerticalSwimlane | ShapeKind::HorizontalSwimlane)
    }

    /// Network device pictures (name drawn below them).
    pub fn is_network_device(self) -> bool {
        use ShapeKind::*;
        matches!(self, Router | Switch | Hub | Server | Firewall | WebFirewall | LoadBalancer | Pc | Laptop | Phone | AccessPoint | Storage | Database | Person)
    }

    /// Small pseudo nodes whose name is shown beside them, not inside.
    pub fn is_pseudo(self) -> bool {
        use ShapeKind::*;
        matches!(
            self,
            Initial
                | Final
                | ActivityFinal
                | FlowFinal
                | Choice
                | Decision
                | Merge
                | Junction
                | Fork
                | Join
                | ShallowHistory
                | DeepHistory
                | EntryPoint
                | ExitPoint
                | Terminate
                | Port
                | ProvidedInterface
                | RequiredInterface
                | AcceptTimeEvent
                | Actor
                | Router
                | Switch
                | Hub
                | Server
                | Firewall
                | WebFirewall
                | LoadBalancer
                | Pc
                | Laptop
                | Phone
                | AccessPoint
                | Storage
                | Database
                | Person
        )
    }

    /// Fixed-size shapes cannot be resized freely.
    pub fn fixed_size(self) -> bool {
        use ShapeKind::*;
        matches!(
            self,
            Initial
                | Final
                | ActivityFinal
                | FlowFinal
                | Junction
                | ShallowHistory
                | DeepHistory
                | EntryPoint
                | ExitPoint
                | Terminate
                | Port
        )
    }

    pub fn has_attributes(self) -> bool {
        use ShapeKind::*;
        matches!(self, Class | Interface | Enumeration | Instance | Entity | DbView | State)
    }

    pub fn has_operations(self) -> bool {
        use ShapeKind::*;
        matches!(self, Class | Interface)
    }

    pub fn is_lifeline(self) -> bool {
        matches!(self, ShapeKind::Lifeline | ShapeKind::ActorLifeline)
    }

    /// What the attribute list is called for this kind of shape.
    pub fn attributes_label(self) -> &'static str {
        use ShapeKind::*;
        match self {
            Enumeration => "Literals",
            Instance => "Slots",
            Entity | DbView => "Columns",
            State => "Activities",
            _ => "Attributes",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ConnKind {
    Association,
    Aggregation,
    Composition,
    Generalization,
    Realization,
    Dependency,
    Usage,
    Anchor,
    Containment,
    Include,
    Extend,
    Message,
    AsyncMessage,
    ReturnMessage,
    CreateMessage,
    Transition,
    ControlFlow,
    ObjectFlow,
    Link,
    Deployment,
    Manifestation,
    Import,
    Access,
    Merge,
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
    Wireless,
    /// A mind map branch from a topic to one of its sub-topics (drawn as a smooth curve).
    Branch,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum End {
    None,
    OpenArrow,
    Triangle,
    FilledArrow,
    HollowDiamond,
    FilledDiamond,
    CircleCross,
    CrowOne,
    CrowMany,
}

pub struct ConnStyle {
    pub dashed: bool,
    pub start: End,
    pub end: End,
    pub keyword: Option<&'static str>,
}

impl ConnKind {
    pub fn style(self) -> ConnStyle {
        use ConnKind::*;
        let s = |dashed, start, end, keyword| ConnStyle { dashed, start, end, keyword };
        match self {
            Association | Link => s(false, End::None, End::None, None),
            Wireless => s(true, End::None, End::None, None),
            Branch => s(false, End::None, End::None, None),
            Aggregation => s(false, End::HollowDiamond, End::None, None),
            Composition => s(false, End::FilledDiamond, End::None, None),
            Generalization => s(false, End::None, End::Triangle, None),
            Realization => s(true, End::None, End::Triangle, None),
            Dependency => s(true, End::None, End::OpenArrow, None),
            Usage => s(true, End::None, End::OpenArrow, Some("use")),
            Anchor => s(true, End::None, End::None, None),
            Containment => s(false, End::None, End::CircleCross, None),
            Include => s(true, End::None, End::OpenArrow, Some("include")),
            Extend => s(true, End::None, End::OpenArrow, Some("extend")),
            Message => s(false, End::None, End::FilledArrow, None),
            AsyncMessage => s(false, End::None, End::OpenArrow, None),
            ReturnMessage => s(true, End::None, End::OpenArrow, None),
            CreateMessage => s(true, End::None, End::OpenArrow, Some("create")),
            Transition | ControlFlow | ObjectFlow => s(false, End::None, End::OpenArrow, None),
            Deployment => s(true, End::None, End::OpenArrow, Some("deploy")),
            Manifestation => s(true, End::None, End::OpenArrow, Some("manifest")),
            Import => s(true, End::None, End::OpenArrow, Some("import")),
            Access => s(true, End::None, End::OpenArrow, Some("access")),
            Merge => s(true, End::None, End::OpenArrow, Some("merge")),
            OneToOne => s(false, End::CrowOne, End::CrowOne, None),
            OneToMany => s(false, End::CrowOne, End::CrowMany, None),
            ManyToOne => s(false, End::CrowMany, End::CrowOne, None),
            ManyToMany => s(false, End::CrowMany, End::CrowMany, None),
        }
    }

    pub fn is_message(self) -> bool {
        use ConnKind::*;
        matches!(self, Message | AsyncMessage | ReturnMessage | CreateMessage)
    }

    pub fn is_erd(self) -> bool {
        use ConnKind::*;
        matches!(self, OneToOne | OneToMany | ManyToOne | ManyToMany)
    }

    /// May a connector of this kind start and end on the same shape?
    /// Association-like lines whose ends have navigability.
    pub fn has_navigability(self) -> bool {
        matches!(self, ConnKind::Association | ConnKind::Aggregation | ConnKind::Composition)
    }

    pub fn allows_self(self) -> bool {
        use ConnKind::*;
        matches!(self, Association | Message | AsyncMessage | Transition | ControlFlow | Dependency | OneToMany | ManyToOne)
    }
}

impl ShapeKind {
    /// Mind map topics (the central topic and the ones branching off it).
    pub fn is_topic(self) -> bool {
        matches!(self, ShapeKind::CentralTopic | ShapeKind::Topic)
    }
}

/// The given mind map topics and every topic below them (following Branch lines from parent to
/// child), the given ones first. Used to move and delete a topic with its sub-topics.
pub fn mind_subtree(d: &Diagram, roots: &[u64]) -> Vec<u64> {
    let mut ids: Vec<u64> = roots.to_vec();
    let mut i = 0;
    while i < ids.len() {
        let id = ids[i];
        for c in &d.connectors {
            if c.kind == ConnKind::Branch && c.from == id && !ids.contains(&c.to) && d.shape(c.to).map(|s| s.kind.is_topic()).unwrap_or(false) {
                ids.push(c.to);
            }
        }
        i += 1;
    }
    ids
}

/// The topic a mind map topic branches off (its parent), if any.
pub fn mind_parent(d: &Diagram, id: u64) -> Option<u64> {
    d.connectors.iter().find(|c| c.kind == ConnKind::Branch && c.to == id && c.from != id).map(|c| c.from)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Shape {
    pub id: u64,
    pub kind: ShapeKind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub name: String,
    #[serde(default)]
    pub stereotype: String,
    #[serde(default)]
    pub attributes: Vec<String>,
    #[serde(default)]
    pub operations: Vec<String>,
    #[serde(default)]
    pub fill: Option<[u8; 4]>,
    #[serde(default)]
    pub documentation: String,
    /// Font family for this shape's text (None = the application's diagram font).
    #[serde(default)]
    pub font: Option<String>,
    /// Text size for this shape (None = the application's text size).
    #[serde(default)]
    pub font_size: Option<f32>,
    /// Which way a required interface (socket) opens: 0 right, 1 down, 2 left, 3 up.
    #[serde(default)]
    pub orient: u8,
    /// Hide the small component icon in a Component's corner.
    #[serde(default)]
    pub hide_icon: bool,
    /// Hide this class's (or interface's) attribute compartment.
    #[serde(default)]
    pub hide_attributes: bool,
    /// Hide this class's (or interface's) operation compartment.
    #[serde(default)]
    pub hide_operations: bool,
    /// Sequence lifelines: heights (diagram units) where the user split its activation; an
    /// activation still open there ends, and the next message starts a new one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub splits: Vec<f32>,
    /// Sequence lifelines: activation stretches the user dragged out, as [top, bottom, depth];
    /// they join the activations they overlap.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub act_spans: Vec<[f32; 3]>,
    /// Alternative combined fragments: the lines between its sections, top to bottom (height
    /// from the fragment's top, in diagram units) with each lower section's guard. None = one
    /// [else] section from halfway down (files from before sections could be changed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dividers: Option<Vec<Divider>>,
    /// Swimlanes: the partitions side by side (left to right, or top to bottom), each with its
    /// width (height) in diagram units; the last one fills what's left. None = a swimlane from
    /// before partitions, shown as one partition named after the shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partitions: Option<Vec<Partition>>,
    /// Where the name shown beside the shape was dragged to (offset from its usual place).
    #[serde(default)]
    pub label_offset: (f32, f32),
    /// A folded mind map topic: its sub-topics (every level) and their lines, kept out of the
    /// diagram until it is unfolded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fold: Option<Box<Fold>>,
    /// A mind map topic: how the branches to its sub-topics are drawn (a `render::STYLE_*`
    /// code from `mindlayout::BRANCH_STYLES`). None = the same as its parent topic's (a root's:
    /// its structure's own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_style: Option<u8>,
}

/// What a folded mind map topic keeps: the shapes below it and every line touching them.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Fold {
    pub shapes: Vec<Shape>,
    pub connectors: Vec<Connector>,
}

impl Fold {
    /// How many topics are folded away, nested folds included.
    pub fn topics(&self) -> usize {
        self.shapes.iter().filter(|s| s.kind.is_topic()).map(|s| 1 + s.fold.as_ref().map(|f| f.topics()).unwrap_or(0)).sum()
    }

    /// The largest id inside, nested folds included.
    pub fn max_id(&self) -> u64 {
        let s = self.shapes.iter().map(|s| s.id.max(s.fold.as_ref().map(|f| f.max_id()).unwrap_or(0))).max().unwrap_or(0);
        s.max(self.connectors.iter().map(|c| c.id).max().unwrap_or(0))
    }
}

/// Whether a mind map topic can fold: a Topic (never the central topic) that branches off
/// another topic and has sub-topics, or is folded already.
pub fn mind_foldable(d: &Diagram, id: u64) -> bool {
    let Some(s) = d.shape(id) else { return false };
    s.kind == ShapeKind::Topic && (s.fold.is_some() || (mind_parent(d, id).is_some() && d.connectors.iter().any(|c| c.kind == ConnKind::Branch && c.from == id && c.to != id && d.shape(c.to).map(|k| k.kind.is_topic()).unwrap_or(false))))
}

/// Fold topic `id`: its sub-topics (every level) leave the diagram with every line touching
/// them and are kept in it. False when there is nothing to fold.
pub fn mind_fold(d: &mut Diagram, id: u64) -> bool {
    if !mind_foldable(d, id) || d.shape(id).map(|s| s.fold.is_some()).unwrap_or(true) {
        return false;
    }
    let below: std::collections::HashSet<u64> = mind_subtree(d, &[id]).into_iter().filter(|x| *x != id).collect();
    if below.is_empty() {
        return false;
    }
    let mut fold = Fold::default();
    let (keep, gone): (Vec<Shape>, Vec<Shape>) = std::mem::take(&mut d.shapes).into_iter().partition(|s| !below.contains(&s.id));
    d.shapes = keep;
    fold.shapes = gone;
    let (keep, gone): (Vec<Connector>, Vec<Connector>) = std::mem::take(&mut d.connectors).into_iter().partition(|c| !below.contains(&c.from) && !below.contains(&c.to));
    d.connectors = keep;
    fold.connectors = gone;
    if let Some(s) = d.shape_mut(id) {
        s.fold = Some(Box::new(fold));
    }
    true
}

/// Unfold topic `id`: what it kept comes back into the diagram. A kept shape or line whose id
/// is taken meanwhile (a pasted copy) gets a new one from `fresh`. False when it isn't folded.
pub fn mind_unfold(d: &mut Diagram, id: u64, mut fresh: impl FnMut() -> u64) -> bool {
    let Some(fold) = d.shape_mut(id).and_then(|s| s.fold.take()) else { return false };
    let Fold { mut shapes, mut connectors } = *fold;
    let mut taken: std::collections::HashSet<u64> = d.shapes.iter().map(|s| s.id).chain(d.connectors.iter().map(|c| c.id)).collect();
    let mut map = std::collections::HashMap::new();
    for s in shapes.iter_mut() {
        if !taken.insert(s.id) {
            let n = fresh();
            map.insert(s.id, n);
            s.id = n;
            taken.insert(n);
        }
    }
    for c in connectors.iter_mut() {
        if !taken.insert(c.id) {
            c.id = fresh();
            taken.insert(c.id);
        }
        if let Some(n) = map.get(&c.from) {
            c.from = *n;
        }
        if let Some(n) = map.get(&c.to) {
            c.to = *n;
        }
    }
    d.shapes.extend(shapes);
    d.connectors.extend(connectors);
    true
}

/// A line between two sections of an alternative combined fragment.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Divider {
    /// Height below the fragment's top, in diagram units.
    pub y: f32,
    /// The guard of the section below the line ("else", a condition, or empty).
    #[serde(default)]
    pub guard: String,
}

/// One partition of a swimlane.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Partition {
    #[serde(default)]
    pub name: String,
    /// Width (vertical swimlane) or height (horizontal), in diagram units.
    pub size: f32,
}

/// Thickness of a swimlane's header strips (its name, and each partition's name).
pub const LANE_HEAD: f32 = 28.0;
/// The narrowest a partition gets.
pub const PART_MIN: f32 = 30.0;

/// Room at the top of an alt fragment for its tab and first guard.
pub const ALT_HEAD: f32 = 34.0;

impl Shape {
    /// A shape with no name, text or styling: everything but its kind and place left empty.
    pub fn new(id: u64, kind: ShapeKind, x: f32, y: f32, w: f32, h: f32) -> Self {
        Shape {
            id,
            kind,
            x,
            y,
            w,
            h,
            name: String::new(),
            stereotype: String::new(),
            attributes: Vec::new(),
            operations: Vec::new(),
            fill: None,
            documentation: String::new(),
            font: None,
            font_size: None,
            orient: 0,
            hide_icon: false,
            hide_attributes: false,
            hide_operations: false,
            fold: None,
            branch_style: None,
            splits: Vec::new(),
            act_spans: Vec::new(),
            dividers: None,
            partitions: None,
            label_offset: (0.0, 0.0),
        }
    }

    /// The section lines of an alternative fragment, top to bottom.
    pub fn alt_dividers(&self) -> Vec<Divider> {
        match &self.dividers {
            Some(v) => v.clone(),
            None if self.kind == ShapeKind::AltFragment => vec![Divider { y: self.h / 2.0, guard: "else".into() }],
            None => vec![],
        }
    }
    fn lane_vertical(&self) -> bool {
        self.kind != ShapeKind::HorizontalSwimlane
    }
    /// A swimlane's length across its partitions (width, or height when horizontal).
    pub fn lane_length(&self) -> f32 {
        if self.lane_vertical() { self.w } else { self.h }
    }
    /// A swimlane's partitions (one named after the shape for swimlanes from before partitions).
    pub fn lane_partitions(&self) -> Vec<Partition> {
        match &self.partitions {
            Some(v) if !v.is_empty() => v.clone(),
            _ => vec![Partition { name: self.name.clone(), size: self.lane_length() }],
        }
    }
    /// Whether the swimlane shows its own name in a header across its partitions.
    pub fn lane_titled(&self) -> bool {
        self.partitions.is_some() && !self.name.trim().is_empty()
    }
    /// Each partition's start and length along the swimlane; the last fills the rest.
    pub fn partition_spans(&self) -> Vec<(f32, f32)> {
        let parts = self.lane_partitions();
        let total = self.lane_length().max(0.0);
        let n = parts.len();
        let mut out = Vec::new();
        let mut at = 0.0;
        for (i, p) in parts.iter().enumerate() {
            let rest = (total - at).max(0.0);
            let len = if i + 1 == n {
                rest
            } else {
                let lo = PART_MIN.min(rest);
                let hi = (rest - (n - i - 1) as f32 * PART_MIN).max(lo);
                p.size.clamp(lo, hi)
            };
            out.push((at, len));
            at += len;
        }
        out
    }
    /// The header strip of partition `i` (x, y, w, h in diagram units).
    pub fn partition_header(&self, i: usize) -> Option<(f32, f32, f32, f32)> {
        let (at, len) = *self.partition_spans().get(i)?;
        let head0 = if self.lane_titled() { LANE_HEAD } else { 0.0 };
        Some(if self.lane_vertical() {
            (self.x + at, self.y + head0, len, LANE_HEAD.min((self.h - head0).max(0.0)))
        } else {
            (self.x + head0, self.y + at, LANE_HEAD.min((self.w - head0).max(0.0)), len)
        })
    }
    /// The header strip with the swimlane's own name, when it has one.
    pub fn lane_title_rect(&self) -> Option<(f32, f32, f32, f32)> {
        if !self.lane_titled() {
            return None;
        }
        Some(if self.lane_vertical() { (self.x, self.y, self.w, LANE_HEAD.min(self.h)) } else { (self.x, self.y, LANE_HEAD.min(self.w), self.h) })
    }
    /// The partition at a distance along the swimlane.
    pub fn partition_at(&self, along: f32) -> usize {
        let spans = self.partition_spans();
        spans.iter().rposition(|(at, _)| *at <= along).unwrap_or(0)
    }
    /// Give a swimlane from before partitions its partition list (its name becomes the
    /// partition's) and make every size the one shown.
    pub fn normalize_partitions(&mut self) {
        if !self.kind.is_swimlane() {
            return;
        }
        let mut v = self.lane_partitions();
        if self.partitions.is_none() {
            self.name.clear();
        }
        for (p, (_, len)) in v.iter_mut().zip(self.partition_spans()) {
            p.size = len;
        }
        self.partitions = Some(v);
    }
    /// Insert a partition after partition `after` (the swimlane grows by its size). Returns
    /// where it starts along the swimlane and its size.
    pub fn add_partition(&mut self, after: usize) -> (f32, f32) {
        self.normalize_partitions();
        let mut v = self.partitions.clone().unwrap_or_default();
        let i = after.min(v.len().saturating_sub(1)) + 1;
        let size = if self.lane_vertical() { 200.0 } else { 150.0 };
        let mut k = 1;
        let name = loop {
            let c = if k == 1 { "Partition".to_string() } else { format!("Partition{k}") };
            if !v.iter().any(|p| p.name == c) {
                break c;
            }
            k += 1;
        };
        let at: f32 = v.iter().take(i).map(|p| p.size).sum();
        v.insert(i.min(v.len()), Partition { name, size });
        if self.lane_vertical() {
            self.w += size;
        } else {
            self.h += size;
        }
        self.partitions = Some(v);
        (at, size)
    }
    /// Take partition `idx` away (the swimlane shrinks by its size; the last one stays).
    /// Returns where it started along the swimlane and its size.
    pub fn remove_partition(&mut self, idx: usize) -> Option<(f32, f32)> {
        if self.lane_partitions().len() < 2 {
            return None;
        }
        self.normalize_partitions();
        let mut v = self.partitions.clone().unwrap_or_default();
        let idx = idx.min(v.len() - 1);
        let at: f32 = v.iter().take(idx).map(|p| p.size).sum();
        let size = v.remove(idx).size;
        if self.lane_vertical() {
            self.w = (self.w - size).max(PART_MIN);
        } else {
            self.h = (self.h - size).max(PART_MIN);
        }
        self.partitions = Some(v);
        Some((at, size))
    }
    /// Move the line between partitions `idx` and `idx + 1` to a distance along the
    /// swimlane; the two partitions share their room (each at least PART_MIN).
    pub fn set_partition_edge(&mut self, idx: usize, along: f32) {
        let spans = self.partition_spans();
        if idx + 1 >= spans.len() {
            return;
        }
        self.normalize_partitions();
        let start = spans[idx].0;
        let end = spans[idx + 1].0 + spans[idx + 1].1;
        let new = (along - start).clamp(PART_MIN, (end - start - PART_MIN).max(PART_MIN));
        if let Some(v) = self.partitions.as_mut() {
            v[idx].size = new;
            v[idx + 1].size = end - start - new;
        }
    }
    pub fn contains_rect(&self, o: &Shape) -> bool {
        o.x >= self.x && o.y >= self.y && o.x + o.w <= self.x + self.w && o.y + o.h <= self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Connector {
    pub id: u64,
    pub kind: ConnKind,
    pub from: u64,
    pub to: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub from_label: String,
    #[serde(default)]
    pub to_label: String,
    /// Vertical position of a sequence-diagram message (diagram units).
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub documentation: String,
    /// Bend points of the line, in diagram units (empty = straight line).
    #[serde(default)]
    pub points: Vec<(f32, f32)>,
    /// Navigability arrow at the source end (associations).
    #[serde(default)]
    pub arrow_start: bool,
    /// Navigability arrow at the target end (associations).
    #[serde(default)]
    pub arrow_end: bool,
    /// Line style, original codes: 0 rectilinear, 1 oblique, 2 curve, 3 follow diagram,
    /// 4 round oblique, 5 round rectilinear (vpplatform.jar v.bjl.u).
    #[serde(default = "style_follow")]
    pub style: u8,
    /// Navigability of the source / target end: 1 True, 2 False, 0 Unspecified
    /// (vpplatform.jar v.bzg.an); default True (v.ddg.r field Oa).
    #[serde(default = "nav_default")]
    pub nav_from: u8,
    #[serde(default = "nav_default")]
    pub nav_to: u8,
    /// A sequence number typed by the user (used when numbering is Manual).
    #[serde(default)]
    pub seq_no: String,
    /// Where the name label was dragged to (offset from the middle of the line).
    #[serde(default)]
    pub label_offset: (f32, f32),
    /// Where the line meets its source / target shape, as a fraction of the shape's width
    /// and height (a point on its outline). None: chosen automatically.
    #[serde(default)]
    pub from_anchor: Option<(f32, f32)>,
    #[serde(default)]
    pub to_anchor: Option<(f32, f32)>,
    /// Where the source / target end labels were dragged to (offset from their usual place).
    #[serde(default, skip_serializing_if = "zero_pair")]
    pub from_label_offset: (f32, f32),
    #[serde(default, skip_serializing_if = "zero_pair")]
    pub to_label_offset: (f32, f32),
}

fn zero_pair(v: &(f32, f32)) -> bool {
    *v == (0.0, 0.0)
}

pub const SEQ_NESTED_LEVEL: u8 = 0;
pub const SEQ_SINGLE_LEVEL: u8 = 1;

pub const NAV_UNSPECIFIED: u8 = 0;
pub const NAV_TRUE: u8 = 1;
pub const NAV_FALSE: u8 = 2;

fn style_follow() -> u8 {
    3
}
fn nav_default() -> u8 {
    NAV_TRUE
}
fn style_oblique() -> u8 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Diagram {
    pub id: u64,
    pub name: String,
    pub kind: DiagramKind,
    pub shapes: Vec<Shape>,
    pub connectors: Vec<Connector>,
    #[serde(default)]
    pub documentation: String,
    /// The folder the diagram is kept in (None = the project root).
    #[serde(default)]
    pub folder: Option<u64>,
    /// Sequence numbers of this diagram; None follows the project (Project Options).
    /// Whether messages are numbered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_messages: Option<bool>,
    /// SEQ_NESTED_LEVEL or SEQ_SINGLE_LEVEL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence_numbering: Option<u8>,
    /// false = automatic, true = manual (a number typed on a message wins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_numbering: Option<bool>,
}

/// A folder for grouping diagrams; folders nest up to 3 levels.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Folder {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub parent: Option<u64>,
}

impl Diagram {
    pub fn shape(&self, id: u64) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }
    pub fn shape_mut(&mut self, id: u64) -> Option<&mut Shape> {
        self.shapes.iter_mut().find(|s| s.id == id)
    }
    pub fn conn_mut(&mut self, id: u64) -> Option<&mut Connector> {
        self.connectors.iter_mut().find(|c| c.id == id)
    }
    /// Messages are numbered: the diagram's own choice, else the project's.
    pub fn numbers_messages(&self, o: &ProjectOptions) -> bool {
        self.number_messages.unwrap_or(o.number_messages)
    }
    /// The sequence number format: the diagram's own choice, else the project's.
    pub fn numbering(&self, o: &ProjectOptions) -> u8 {
        self.sequence_numbering.unwrap_or(o.sequence_numbering)
    }
    /// Manual sequence numbers: the diagram's own choice, else the project's.
    pub fn manual(&self, o: &ProjectOptions) -> bool {
        self.manual_numbering.unwrap_or(o.manual_numbering)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErdNotation {
    CrowsFoot,
    Multiplicity,
}

/// Per-project diagramming options (Window > Project Options).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectOptions {
    pub show_attributes: bool,
    pub show_operations: bool,
    pub show_stereotypes: bool,
    pub show_multiplicity: bool,
    pub number_messages: bool,
    pub erd_notation: ErdNotation,
    pub default_visibility: char,
    pub auto_fit_shapes: bool,
    /// Diagram connector style; DefaultConnectorStyle default 1 = Oblique (v.ddg.r field Hx).
    #[serde(default = "style_oblique")]
    pub connector_style: u8,
    /// sequenceNumbering, original codes (openapi IInteractionDiagramUIModel):
    /// 0 NESTED_LEVEL (default, v.cor.az.<init>), 1 SINGLE_LEVEL,
    /// 2 FRAME_BASED_NESTED_LEVEL, 3 FRAME_BASED_SINGLE_LEVEL (2 and 3 not offered here).
    #[serde(default)]
    pub sequence_numbering: u8,
    /// sequenceNumberHandling: false = AUTOMATIC (0, default), true = MANUAL (1).
    #[serde(default)]
    pub manual_numbering: bool,
    /// autoExtendActivations; default true (v.ddg.r field RR, read by jv()).
    #[serde(default = "yes")]
    pub auto_extend_activations: bool,
    /// Draw activation bars on lifelines.
    #[serde(default = "yes")]
    pub show_activations: bool,
    /// Canvas colour (the original's DefaultDiagramBackground is white).
    #[serde(default = "white")]
    pub diagram_background: [u8; 3],
    /// Fill of new shapes. White as the person asked; the original's default is
    /// RGB(122, 207, 245) (v.ddg.r DefaultShapeBackground).
    #[serde(default = "white")]
    pub shape_fill: [u8; 3],
    /// Text colour (the original's DefaultFontColor is black).
    #[serde(default = "black")]
    pub text_color: [u8; 3],
    /// Font family of the project's diagram text ("" = the application's diagram font).
    #[serde(default)]
    pub font: String,
}

fn white() -> [u8; 3] {
    [255, 255, 255]
}
fn black() -> [u8; 3] {
    [0, 0, 0]
}

fn yes() -> bool {
    true
}

impl Default for ProjectOptions {
    fn default() -> Self {
        ProjectOptions {
            show_attributes: true,
            show_operations: true,
            show_stereotypes: true,
            show_multiplicity: true,
            number_messages: true,
            erd_notation: ErdNotation::CrowsFoot,
            default_visibility: '-',
            auto_fit_shapes: true,
            connector_style: 1,
            sequence_numbering: SEQ_NESTED_LEVEL,
            manual_numbering: false,
            auto_extend_activations: true,
            show_activations: true,
            diagram_background: white(),
            shape_fill: white(),
            text_color: black(),
            font: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub diagrams: Vec<Diagram>,
    #[serde(default)]
    pub folders: Vec<Folder>,
    #[serde(default)]
    pub options: ProjectOptions,
    pub next_id: u64,
}

impl Project {
    pub fn new(name: &str) -> Self {
        Project {
            name: name.to_string(),
            author: String::new(),
            description: String::new(),
            diagrams: Vec::new(),
            folders: Vec::new(),
            options: ProjectOptions::default(),
            next_id: 1,
        }
    }

    pub fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn diagram(&self, id: u64) -> Option<&Diagram> {
        self.diagrams.iter().find(|d| d.id == id)
    }
    pub fn diagram_mut(&mut self, id: u64) -> Option<&mut Diagram> {
        self.diagrams.iter_mut().find(|d| d.id == id)
    }

    pub fn folder(&self, id: u64) -> Option<&Folder> {
        self.folders.iter().find(|f| f.id == id)
    }

    pub fn add_folder(&mut self, name: &str, parent: Option<u64>) -> u64 {
        let id = self.fresh_id();
        self.folders.push(Folder { id, name: name.to_string(), parent });
        id
    }

    /// Next default diagram name, e.g. "Class Diagram1".
    pub fn next_diagram_name(&self, kind: DiagramKind) -> String {
        let base = kind.label();
        let mut n = 1;
        loop {
            let name = format!("{base}{n}");
            if !self.diagrams.iter().any(|d| d.name == name) {
                return name;
            }
            n += 1;
        }
    }

    pub fn add_diagram(&mut self, kind: DiagramKind, name: Option<String>) -> u64 {
        let id = self.fresh_id();
        let name = name.unwrap_or_else(|| self.next_diagram_name(kind));
        self.diagrams.push(Diagram { id, name, kind, shapes: vec![], connectors: vec![], documentation: String::new(), folder: None, number_messages: None, sequence_numbering: None, manual_numbering: None });
        id
    }

    /// Make sure ids continue after the largest id in a loaded file.
    pub fn repair_ids(&mut self) {
        let mut max = self.folders.iter().map(|f| f.id).max().unwrap_or(0);
        for d in &self.diagrams {
            max = max.max(d.id);
            for s in &d.shapes {
                max = max.max(s.id).max(s.fold.as_ref().map(|f| f.max_id()).unwrap_or(0));
            }
            for c in &d.connectors {
                max = max.max(c.id);
            }
        }
        if self.next_id <= max {
            self.next_id = max + 1;
        }
    }

    /// The example project shown on first start: one small example of every diagram type.
    pub fn sample() -> Self {
        use ConnKind as C;
        use ShapeKind as S;
        let mut p = Project::new("Example");

        // 1. class diagram
        let mut b = Builder::new(&mut p, DiagramKind::Class, "Ordering");
        let customer = b.shape(S::Class, 40.0, 40.0, 150.0, "Customer", &["-name : String", "-email : String"], &["+placeOrder() : Order"]);
        let order = b.shape(S::Class, 320.0, 40.0, 160.0, "Order", &["-date : Date", "-status : OrderStatus"], &["+total() : double"]);
        let status = b.shape(S::Enumeration, 590.0, 40.0, 130.0, "OrderStatus", &["NEW", "PAID", "SHIPPED"], &[]);
        let line = b.shape(S::Class, 320.0, 260.0, 160.0, "OrderLine", &["-quantity : int"], &["+subtotal() : double"]);
        let payable = b.shape(S::Interface, 40.0, 260.0, 170.0, "Payable", &[], &["+pay(amount : double) : void"]);
        let product = b.shape(S::Class, 590.0, 260.0, 130.0, "Product", &["-name : String", "-price : double"], &[]);
        b.conn(C::Association, customer, order, "", "1", "0..*");
        b.conn(C::Composition, order, line, "", "1", "1..*");
        b.conn(C::Dependency, order, status, "", "", "");
        b.conn(C::Realization, order, payable, "", "", "");
        b.conn(C::Association, line, product, "", "*", "1");
        b.arrow_last();

        // 2. component diagram
        let mut b = Builder::new(&mut p, DiagramKind::Component, "Shop Components");
        let ui = b.shape(S::Component, 30.0, 90.0, 140.0, "Web UI", &[], &[]);
        // a component holding two components
        let svc = b.shape_h(S::Component, 300.0, 40.0, 280.0, 190.0, "Order Service");
        let api = b.shape_h(S::Component, 315.0, 100.0, 120.0, 60.0, "Order API");
        let logic = b.shape_h(S::Component, 450.0, 100.0, 115.0, 60.0, "Order Logic");
        // Web UI needs IOrder (socket), Order Service offers it (ball)
        let need = b.shape_h(S::RequiredInterface, 210.0, 107.0, 36.0, 36.0, "");
        let give = b.shape_h(S::ProvidedInterface, 216.0, 113.0, 24.0, 24.0, "IOrder");
        let pay = b.shape(S::Component, 300.0, 300.0, 170.0, "Payment Gateway", &[], &[]);
        let db = b.shape(S::Component, 630.0, 100.0, 120.0, "Database", &[], &[]);
        b.conn(C::Usage, ui, need, "", "", "");
        b.conn(C::Realization, api, give, "", "", "");
        b.conn(C::Usage, api, logic, "", "", "");
        b.conn(C::Dependency, svc, pay, "", "", "");
        b.conn_bent(C::Dependency, logic, db, &[(507.0, 60.0), (690.0, 60.0)]);

        // 3. deployment diagram
        let mut b = Builder::new(&mut p, DiagramKind::Deployment, "Shop Deployment");
        let pc = b.shape_h(S::Device, 30.0, 60.0, 200.0, 170.0, "Client PC");
        b.shape_h(S::ExecutionEnvironment, 45.0, 110.0, 160.0, 100.0, "Web Browser");
        let server = b.shape_h(S::Node, 300.0, 60.0, 250.0, 220.0, "Application Server");
        let comp = b.shape(S::Component, 320.0, 120.0, 190.0, "Order Service", &[], &[]);
        let war = b.shape(S::Artifact, 320.0, 205.0, 190.0, "shop.war", &[], &[]);
        let dbs = b.shape_h(S::Node, 300.0, 330.0, 250.0, 130.0, "Database Server");
        b.conn(C::Link, pc, server, "HTTPS", "", "");
        b.conn(C::Link, server, dbs, "JDBC", "", "");
        b.conn(C::Manifestation, war, comp, "", "", "");

        // 4. use case diagram
        let mut b = Builder::new(&mut p, DiagramKind::UseCase, "Shop Use Cases");
        let cust = b.shape_h(S::Actor, 40.0, 170.0, 40.0, 80.0, "Customer");
        b.shape_h(S::System, 150.0, 20.0, 380.0, 440.0, "Web Shop");
        let browse = b.shape_h(S::UseCase, 230.0, 70.0, 150.0, 60.0, "Browse Products");
        let place = b.shape_h(S::UseCase, 230.0, 180.0, 150.0, 60.0, "Place Order");
        let login = b.shape_h(S::UseCase, 360.0, 300.0, 140.0, 60.0, "Log In");
        let coupon = b.shape_h(S::UseCase, 180.0, 360.0, 150.0, 60.0, "Apply Coupon");
        let payuc = b.shape_h(S::UseCase, 370.0, 120.0, 130.0, 60.0, "Pay");
        let bank = b.shape_h(S::Actor, 620.0, 120.0, 40.0, 80.0, "Payment Service");
        b.conn(C::Association, cust, browse, "", "", "");
        b.conn(C::Association, cust, place, "", "", "");
        b.conn(C::Include, place, login, "", "", "");
        b.conn(C::Include, place, payuc, "", "", "");
        b.conn(C::Extend, coupon, place, "", "", "");
        b.conn(C::Association, payuc, bank, "", "", "");

        // 5. sequence diagram
        let mut b = Builder::new(&mut p, DiagramKind::Sequence, "Place Order");
        let a = b.shape_h(S::ActorLifeline, 30.0, 20.0, 90.0, 400.0, "Customer");
        let w = b.shape_h(S::Lifeline, 180.0, 20.0, 110.0, 400.0, ":WebUI");
        let o = b.shape_h(S::Lifeline, 350.0, 20.0, 130.0, 400.0, ":OrderService");
        let g = b.shape_h(S::Lifeline, 550.0, 20.0, 130.0, 400.0, ":PaymentGateway");
        b.shape_h(S::AltFragment, 300.0, 165.0, 420.0, 170.0, "amount > 0");
        b.msg(C::Message, a, w, 120.0, "placeOrder()");
        b.msg(C::Message, w, o, 150.0, "createOrder(items)");
        b.msg(C::Message, o, g, 215.0, "charge(amount)");
        let charge = b.last_conn();
        b.msg(C::ReturnMessage, g, o, 240.0, "ok");
        b.msg(C::AsyncMessage, o, o, 285.0, "sendReceipt()");
        b.msg(C::ReturnMessage, o, w, 350.0, "orderId");
        b.msg(C::ReturnMessage, w, a, 380.0, "confirmation");
        let note = b.shape_h(S::Note, 540.0, 440.0, 190.0, 46.0, "The payment is charged before the receipt is sent.");
        b.conn(C::Anchor, note, charge, "", "", "");

        // 6. state machine diagram
        let mut b = Builder::new(&mut p, DiagramKind::StateMachine, "Order States");
        let init = b.shape_h(S::Initial, 40.0, 138.0, 24.0, 24.0, "");
        let new = b.shape_h(S::State, 110.0, 120.0, 120.0, 60.0, "New");
        let paid = b.shape_h(S::State, 320.0, 120.0, 120.0, 60.0, "Paid");
        let shipped = b.shape_h(S::State, 530.0, 120.0, 120.0, 60.0, "Shipped");
        let fin = b.shape_h(S::Final, 680.0, 137.0, 26.0, 26.0, "");
        let cancelled = b.shape_h(S::State, 320.0, 290.0, 120.0, 60.0, "Cancelled");
        let fin2 = b.shape_h(S::Final, 367.0, 410.0, 26.0, 26.0, "");
        b.conn(C::Transition, init, new, "", "", "");
        b.conn(C::Transition, new, paid, "pay", "", "");
        b.conn(C::Transition, paid, shipped, "ship", "", "");
        b.conn(C::Transition, shipped, fin, "deliver", "", "");
        b.conn(C::Transition, new, cancelled, "cancel", "", "");
        b.conn(C::Transition, cancelled, fin2, "", "", "");

        // 7. activity diagram
        let mut b = Builder::new(&mut p, DiagramKind::Activity, "Checkout");
        let lane = b.shape_h(S::VerticalSwimlane, 30.0, 10.0, 540.0, 480.0, "");
        b.partitions(lane, &[("Customer", 270.0), ("Shop", 270.0)]);
        let i = b.shape_h(S::Initial, 153.0, 60.0, 24.0, 24.0, "");
        let choose = b.shape_h(S::Action, 95.0, 110.0, 140.0, 50.0, "Choose Products");
        let enter = b.shape_h(S::Action, 95.0, 200.0, 140.0, 50.0, "Enter Payment");
        let check = b.shape_h(S::Action, 365.0, 200.0, 140.0, 50.0, "Check Payment");
        let dec = b.shape_h(S::Decision, 419.0, 290.0, 32.0, 32.0, "");
        let ship = b.shape_h(S::Action, 365.0, 360.0, 140.0, 50.0, "Ship Order");
        let end = b.shape_h(S::ActivityFinal, 422.0, 445.0, 26.0, 26.0, "");
        b.conn(C::ControlFlow, i, choose, "", "", "");
        b.conn(C::ControlFlow, choose, enter, "", "", "");
        b.conn(C::ControlFlow, enter, check, "", "", "");
        b.conn(C::ControlFlow, check, dec, "", "", "");
        b.conn(C::ControlFlow, dec, ship, "[accepted]", "", "");
        b.conn(C::ControlFlow, dec, enter, "[declined]", "", "");
        b.conn(C::ControlFlow, ship, end, "", "", "");

        // 8. object diagram
        let mut b = Builder::new(&mut p, DiagramKind::Object, "Order Snapshot");
        let c1 = b.shape(S::Instance, 40.0, 60.0, 170.0, "kim : Customer", &["name = \"Kim\"", "email = \"kim@example.com\""], &[]);
        let o1 = b.shape(S::Instance, 320.0, 60.0, 160.0, "o42 : Order", &["status = PAID", "date = 2026-11-13"], &[]);
        let l1 = b.shape(S::Instance, 320.0, 260.0, 160.0, "l1 : OrderLine", &["quantity = 2"], &[]);
        let l2 = b.shape(S::Instance, 560.0, 260.0, 160.0, "l2 : OrderLine", &["quantity = 1"], &[]);
        b.conn(C::Link, c1, o1, "", "", "");
        b.conn(C::Link, o1, l1, "", "", "");
        b.conn(C::Link, o1, l2, "", "", "");

        // 9. package diagram
        let mut b = Builder::new(&mut p, DiagramKind::Package, "Layers");
        let pui = b.shape_h(S::Package, 40.0, 60.0, 160.0, 110.0, "ui");
        let pdom = b.shape_h(S::Package, 320.0, 60.0, 160.0, 110.0, "domain");
        let ppers = b.shape_h(S::Package, 320.0, 280.0, 160.0, 110.0, "persistence");
        let putil = b.shape_h(S::Package, 40.0, 280.0, 160.0, 110.0, "util");
        let pext = b.shape_h(S::Subsystem, 580.0, 60.0, 160.0, 110.0, "payment");
        b.conn(C::Dependency, pui, pdom, "", "", "");
        b.conn(C::Dependency, pdom, ppers, "", "", "");
        b.conn(C::Import, pui, putil, "", "", "");
        b.conn(C::Access, pdom, pext, "", "", "");

        // 10. entity relationship diagram
        let mut b = Builder::new(&mut p, DiagramKind::Erd, "Shop Database");
        let tc = b.shape(S::Entity, 40.0, 40.0, 190.0, "customer", &["PK id : integer", "name : varchar(100)", "email : varchar(255)"], &[]);
        let to = b.shape(S::Entity, 330.0, 40.0, 190.0, "orders", &["PK id : integer", "FK customer_id : integer", "order_date : date"], &[]);
        let tl = b.shape(S::Entity, 330.0, 260.0, 190.0, "order_line", &["PK id : integer", "FK order_id : integer", "FK product_id : integer", "quantity : integer"], &[]);
        let tp = b.shape(S::Entity, 40.0, 260.0, 190.0, "product", &["PK id : integer", "name : varchar(100)", "price : numeric(10,2)"], &[]);
        b.conn(C::OneToMany, tc, to, "", "", "");
        b.conn(C::OneToMany, to, tl, "", "", "");
        b.conn(C::OneToMany, tp, tl, "", "", "");

        // 11. network diagram
        let mut b = Builder::new(&mut p, DiagramKind::Network, "Office Network");
        let dmz = b.shape_h(S::NetworkZone, 330.0, 330.0, 300.0, 140.0, "DMZ");
        let web = b.shape_h(S::NetworkZone, 330.0, 520.0, 420.0, 150.0, "Web Servers");
        let lan = b.shape_h(S::NetworkZone, 20.0, 330.0, 270.0, 340.0, "Office LAN");
        let _ = (dmz, web, lan);
        let net = b.shape_h(S::Cloud, 380.0, 20.0, 130.0, 70.0, "Internet");
        let rt = b.shape_h(S::Router, 415.0, 130.0, 60.0, 44.0, "Edge Router");
        let fw = b.shape_h(S::Firewall, 415.0, 230.0, 60.0, 48.0, "Firewall");
        let waf = b.shape_h(S::WebFirewall, 370.0, 375.0, 60.0, 48.0, "WAF");
        let lb = b.shape_h(S::LoadBalancer, 520.0, 379.0, 64.0, 40.0, "LB");
        let w1 = b.shape_h(S::Server, 390.0, 565.0, 44.0, 64.0, "Web 1");
        let w2 = b.shape_h(S::Server, 520.0, 565.0, 44.0, 64.0, "Web 2");
        let sw = b.shape_h(S::Switch, 120.0, 380.0, 70.0, 34.0, "Switch");
        let pc = b.shape_h(S::Pc, 40.0, 470.0, 56.0, 50.0, "PC");
        let ap = b.shape_h(S::AccessPoint, 180.0, 470.0, 50.0, 40.0, "Wi-Fi AP");
        let lt = b.shape_h(S::Laptop, 120.0, 580.0, 60.0, 44.0, "Laptop");
        let ph = b.shape_h(S::Phone, 230.0, 575.0, 30.0, 52.0, "Phone");
        let db = b.shape_h(S::Database, 650.0, 568.0, 50.0, 58.0, "Orders DB");
        let _user = b.shape_h(S::Person, 48.0, 580.0, 40.0, 50.0, "Staff");
        b.conn(C::Link, net, rt, "", "", "");
        b.conn(C::Link, rt, fw, "", "", "");
        b.conn(C::Link, fw, waf, "", "", "");
        b.conn(C::Link, waf, lb, "", "", "");
        b.conn(C::Link, lb, w1, "", "", "");
        b.conn(C::Link, lb, w2, "", "", "");
        b.conn(C::Link, w2, db, "", "", "");
        b.conn(C::Link, fw, sw, "", "", "");
        b.conn(C::Link, sw, pc, "", "", "");
        b.conn(C::Link, sw, ap, "", "", "");
        b.conn(C::Wireless, ap, lt, "", "", "");
        b.conn(C::Wireless, ap, ph, "", "", "");

        // 12. mind map
        let mut b = Builder::new(&mut p, DiagramKind::MindMap, "Shop Ideas");
        let mc = b.shape_h(S::CentralTopic, 330.0, 228.0, 170.0, 64.0, "Online Shop");
        let m1 = b.shape_h(S::Topic, 560.0, 110.0, 120.0, 36.0, "Customers");
        let m2 = b.shape_h(S::Topic, 560.0, 300.0, 120.0, 36.0, "Products");
        let m3 = b.shape_h(S::Topic, 150.0, 150.0, 120.0, 36.0, "Payment");
        let m4 = b.shape_h(S::Topic, 150.0, 330.0, 120.0, 36.0, "Delivery");
        let m11 = b.shape_h(S::Topic, 730.0, 70.0, 120.0, 36.0, "Sign up");
        let m12 = b.shape_h(S::Topic, 730.0, 120.0, 120.0, 36.0, "Reviews");
        let m21 = b.shape_h(S::Topic, 730.0, 280.0, 120.0, 36.0, "Catalog");
        let m22 = b.shape_h(S::Topic, 730.0, 330.0, 120.0, 36.0, "Stock");
        let m31 = b.shape_h(S::Topic, -20.0, 120.0, 120.0, 36.0, "Card");
        let m32 = b.shape_h(S::Topic, -20.0, 170.0, 120.0, 36.0, "Bank transfer");
        for (a, z) in [(mc, m1), (mc, m2), (mc, m3), (mc, m4), (m1, m11), (m1, m12), (m2, m21), (m2, m22), (m3, m31), (m3, m32)] {
            b.conn(C::Branch, a, z, "", "", "");
        }

        // folders: the diagrams grouped the way a team might organise them
        let req = p.add_folder("0-Requirements", None);
        let arch = p.add_folder("1-Architecture", None);
        let design = p.add_folder("2-Design", None);
        let behavior = p.add_folder("2.1-Behavior", Some(design));
        let db = p.add_folder("3-Database", None);
        let plan = [design, arch, arch, req, behavior, behavior, behavior, design, arch, db, arch, req];
        for (d, f) in p.diagrams.iter_mut().zip(plan) {
            d.folder = Some(f);
        }

        p
    }
}

/// Helper for building the example diagrams.
struct Builder<'a> {
    p: &'a mut Project,
    did: u64,
}

impl<'a> Builder<'a> {
    fn new(p: &'a mut Project, kind: DiagramKind, name: &str) -> Self {
        let did = p.add_diagram(kind, Some(name.to_string()));
        Builder { p, did }
    }

    fn push(&mut self, kind: ShapeKind, x: f32, y: f32, w: f32, h: f32, name: &str, attrs: &[&str], ops: &[&str]) -> u64 {
        let id = self.p.fresh_id();
        let d = self.p.diagram_mut(self.did).unwrap();
        d.shapes.push(Shape {
            name: name.to_string(),
            attributes: attrs.iter().map(|s| s.to_string()).collect(),
            operations: ops.iter().map(|s| s.to_string()).collect(),
            ..Shape::new(id, kind, x, y, w, h)
        });
        id
    }

    fn shape(&mut self, kind: ShapeKind, x: f32, y: f32, w: f32, name: &str, attrs: &[&str], ops: &[&str]) -> u64 {
        let (_, h) = kind.default_size();
        self.push(kind, x, y, w, h, name, attrs, ops)
    }

    fn shape_h(&mut self, kind: ShapeKind, x: f32, y: f32, w: f32, h: f32, name: &str) -> u64 {
        self.push(kind, x, y, w, h, name, &[], &[])
    }

    fn partitions(&mut self, id: u64, parts: &[(&str, f32)]) {
        if let Some(s) = self.p.diagram_mut(self.did).unwrap().shape_mut(id) {
            s.partitions = Some(parts.iter().map(|(n, w)| Partition { name: n.to_string(), size: *w }).collect());
        }
    }

    fn conn(&mut self, kind: ConnKind, from: u64, to: u64, name: &str, fl: &str, tl: &str) {
        self.msg_full(kind, from, to, 0.0, name, fl, tl);
    }

    fn last_conn(&mut self) -> u64 {
        self.p.diagram_mut(self.did).unwrap().connectors.last().map(|c| c.id).unwrap_or(0)
    }

    fn conn_bent(&mut self, kind: ConnKind, from: u64, to: u64, points: &[(f32, f32)]) {
        self.conn(kind, from, to, "", "", "");
        if let Some(c) = self.p.diagram_mut(self.did).unwrap().connectors.last_mut() {
            c.points = points.to_vec();
        }
    }

    /// Make the last connector navigable only toward its target (an arrow there).
    fn arrow_last(&mut self) {
        if let Some(c) = self.p.diagram_mut(self.did).unwrap().connectors.last_mut() {
            c.nav_to = NAV_TRUE;
            c.nav_from = NAV_UNSPECIFIED;
        }
    }

    fn msg(&mut self, kind: ConnKind, from: u64, to: u64, y: f32, name: &str) {
        self.msg_full(kind, from, to, y, name, "", "");
    }

    fn msg_full(&mut self, kind: ConnKind, from: u64, to: u64, y: f32, name: &str, fl: &str, tl: &str) {
        let id = self.p.fresh_id();
        self.p.diagram_mut(self.did).unwrap().connectors.push(Connector {
            id,
            kind,
            from,
            to,
            name: name.to_string(),
            from_label: fl.to_string(),
            to_label: tl.to_string(),
            y,
            documentation: String::new(),
            points: vec![],
            arrow_start: false,
            arrow_end: false,
            style: 3,
            nav_from: NAV_TRUE,
            nav_to: NAV_TRUE,
            seq_no: String::new(),
            from_label_offset: (0.0, 0.0),
            to_label_offset: (0.0, 0.0),
            label_offset: (0.0, 0.0),
            from_anchor: None,
            to_anchor: None,
        });
    }
}

/// When a line is drawn between two side-by-side (or stacked) boxes and the drag starts
/// away from the middle of the box, the line runs straight across at that height (or
/// position): both ends are fixed there. Starting near the middle leaves the ends free.
pub fn press_lane(a: &Shape, b: &Shape, press: (f32, f32)) -> Option<((f32, f32), (f32, f32))> {
    let boxy = |s: &Shape| s.kind.is_network_device() || !(s.kind.is_pseudo() || matches!(s.kind, ShapeKind::UseCase | ShapeKind::Collaboration | ShapeKind::Choice | ShapeKind::Decision | ShapeKind::Merge));
    if !boxy(a) || !boxy(b) {
        return None;
    }
    if a.x + a.w <= b.x || b.x + b.w <= a.x {
        let (lo, hi) = (a.y.max(b.y), (a.y + a.h).min(b.y + b.h));
        if hi - lo < 8.0 || (press.1 - (a.y + a.h / 2.0)).abs() < a.h * 0.15 {
            return None;
        }
        let y = press.1.clamp(lo + 3.0, hi - 3.0);
        let a_left = a.x + a.w <= b.x;
        return Some(((if a_left { 1.0 } else { 0.0 }, (y - a.y) / a.h), (if a_left { 0.0 } else { 1.0 }, (y - b.y) / b.h)));
    }
    if a.y + a.h <= b.y || b.y + b.h <= a.y {
        let (lo, hi) = (a.x.max(b.x), (a.x + a.w).min(b.x + b.w));
        if hi - lo < 8.0 || (press.0 - (a.x + a.w / 2.0)).abs() < a.w * 0.15 {
            return None;
        }
        let x = press.0.clamp(lo + 3.0, hi - 3.0);
        let a_top = a.y + a.h <= b.y;
        return Some((((x - a.x) / a.w, if a_top { 1.0 } else { 0.0 }), ((x - b.x) / b.w, if a_top { 0.0 } else { 1.0 })));
    }
    None
}

/// Shapes that ports attach to.
pub fn takes_ports(k: ShapeKind) -> bool {
    matches!(k, ShapeKind::Component | ShapeKind::Node | ShapeKind::Device | ShapeKind::ExecutionEnvironment | ShapeKind::Class | ShapeKind::Instance)
}

/// Ports sitting on (or inside) a shape's outline: their centre is within half a port of it.
pub fn attached_ports(d: &Diagram, owner: &Shape) -> Vec<u64> {
    if !takes_ports(owner.kind) {
        return vec![];
    }
    d.shapes
        .iter()
        .filter(|p| p.kind == ShapeKind::Port && p.id != owner.id)
        .filter(|p| {
            let (cx, cy) = p.center();
            let m = p.w.max(p.h) / 2.0 + 2.0;
            cx >= owner.x - m && cx <= owner.x + owner.w + m && cy >= owner.y - m && cy <= owner.y + owner.h + m
        })
        .map(|p| p.id)
        .collect()
}

pub fn is_interface_end(k: ShapeKind) -> bool {
    matches!(k, ShapeKind::ProvidedInterface | ShapeKind::RequiredInterface)
}

pub fn opposite_interface(a: ShapeKind, b: ShapeKind) -> bool {
    is_interface_end(a) && is_interface_end(b) && a != b
}

/// A provided interface (ball) and a required interface (socket) whose centres sit on each
/// other are joined: the ball rests in the socket and they move as one.
pub fn joined_interfaces(d: &Diagram, s: &Shape) -> Vec<u64> {
    if !is_interface_end(s.kind) {
        return vec![];
    }
    let (cx, cy) = s.center();
    d.shapes
        .iter()
        .filter(|o| o.id != s.id && opposite_interface(s.kind, o.kind))
        .filter(|o| {
            let (ox, oy) = o.center();
            (ox - cx).abs() <= 1.5 && (oy - cy).abs() <= 1.5
        })
        .map(|o| o.id)
        .collect()
}

/// The smallest port-taking shape near a port (its outline within 12 px of the port centre).
pub fn port_owner(d: &Diagram, port: &Shape) -> Option<u64> {
    let (cx, cy) = port.center();
    d.shapes
        .iter()
        .filter(|s| takes_ports(s.kind) && s.id != port.id)
        .filter(|s| cx >= s.x - 12.0 && cx <= s.x + s.w + 12.0 && cy >= s.y - 12.0 && cy <= s.y + s.h + 12.0)
        .min_by(|a, b| (a.w * a.h).partial_cmp(&(b.w * b.h)).unwrap_or(std::cmp::Ordering::Equal))
        .map(|s| s.id)
}

/// Put a port's centre on the nearest edge of its owner.
pub fn snap_port(port: &mut Shape, owner: &Shape) {
    let (cx, cy) = port.center();
    let x = cx.clamp(owner.x, owner.x + owner.w);
    let y = cy.clamp(owner.y, owner.y + owner.h);
    let dl = (x - owner.x).abs();
    let dr = (owner.x + owner.w - x).abs();
    let dt = (y - owner.y).abs();
    let db = (owner.y + owner.h - y).abs();
    let m = dl.min(dr).min(dt).min(db);
    let (nx, ny) = if m == dl {
        (owner.x, y)
    } else if m == dr {
        (owner.x + owner.w, y)
    } else if m == dt {
        (x, owner.y)
    } else {
        (x, owner.y + owner.h)
    };
    port.x = nx - port.w / 2.0;
    port.y = ny - port.h / 2.0;
}

/// Move a section line of an alternative fragment to height `ny` (diagram units), keeping it
/// 20 below the line above (or below the fragment's tab) and 20 above the line below (10 above
/// the bottom for the last).
pub fn set_divider(s: &mut Shape, idx: usize, ny: f32) {
    let mut v = s.alt_dividers();
    if idx < v.len() {
        let lo = if idx == 0 { ALT_HEAD } else { v[idx - 1].y + 20.0 };
        let hi = if idx + 1 < v.len() { v[idx + 1].y - 20.0 } else { s.h - 10.0 };
        v[idx].y = (ny - s.y).clamp(lo, hi.max(lo));
        s.dividers = Some(v);
    }
}

/// Move what stands inside a swimlane (as it was before a change) beyond a distance along it
/// by `delta`: shapes whose middle is past it and bend points past it.
pub fn shift_lane_contents(dm: &mut Diagram, lane: &Shape, from: f32, delta: f32) {
    let vertical = lane.kind != ShapeKind::HorizontalSwimlane;
    let cut = if vertical { lane.x + from } else { lane.y + from };
    let inside = |x: f32, y: f32| x >= lane.x && x <= lane.x + lane.w && y >= lane.y && y <= lane.y + lane.h;
    let beyond = |x: f32, y: f32| if vertical { x >= cut } else { y >= cut };
    for s in dm.shapes.iter_mut().filter(|s| s.id != lane.id && !s.kind.is_swimlane()) {
        let (cx, cy) = s.center();
        if inside(cx, cy) && beyond(cx, cy) {
            if vertical {
                s.x += delta;
            } else {
                s.y += delta;
            }
        }
    }
    for c in dm.connectors.iter_mut() {
        for q in c.points.iter_mut() {
            if inside(q.0, q.1) && beyond(q.0, q.1) {
                if vertical {
                    q.0 += delta;
                } else {
                    q.1 += delta;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagram(kind: DiagramKind, shapes: Vec<Shape>) -> Diagram {
        let mut p = Project::new("t");
        let did = p.add_diagram(kind, None);
        let mut d = p.diagram_mut(did).unwrap().clone();
        d.shapes = shapes;
        d
    }

    #[test]
    fn port_owner_is_the_smallest_shape_near_the_port() {
        let d = diagram(DiagramKind::Component, vec![
            Shape::new(1, ShapeKind::Component, 0.0, 0.0, 200.0, 200.0),
            Shape::new(2, ShapeKind::Class, 50.0, 50.0, 60.0, 60.0),
            Shape::new(3, ShapeKind::Port, 45.0, 45.0, 10.0, 10.0),
            Shape::new(4, ShapeKind::Port, 500.0, 500.0, 10.0, 10.0),
        ]);
        assert_eq!(port_owner(&d, &d.shapes[2]), Some(2));
        assert_eq!(port_owner(&d, &d.shapes[3]), None);
    }

    #[test]
    fn snap_port_moves_to_the_nearest_edge() {
        let owner = Shape::new(1, ShapeKind::Component, 0.0, 0.0, 100.0, 100.0);
        let mut port = Shape::new(2, ShapeKind::Port, 5.0, 40.0, 10.0, 10.0);
        snap_port(&mut port, &owner);
        assert_eq!((port.x, port.y), (-5.0, 40.0));
        let mut port = Shape::new(3, ShapeKind::Port, 40.0, 88.0, 10.0, 10.0);
        snap_port(&mut port, &owner);
        assert_eq!((port.x, port.y), (40.0, 95.0));
    }

    #[test]
    fn attached_ports_only_on_port_taking_shapes() {
        let d = diagram(DiagramKind::Component, vec![
            Shape::new(1, ShapeKind::Component, 0.0, 0.0, 100.0, 100.0),
            Shape::new(2, ShapeKind::Note, 0.0, 0.0, 100.0, 100.0),
            Shape::new(3, ShapeKind::Port, 95.0, 40.0, 10.0, 10.0),
            Shape::new(4, ShapeKind::Port, 300.0, 40.0, 10.0, 10.0),
        ]);
        assert_eq!(attached_ports(&d, &d.shapes[0]), vec![3]);
        assert!(attached_ports(&d, &d.shapes[1]).is_empty());
    }

    #[test]
    fn ball_and_socket_on_each_other_are_joined() {
        let d = diagram(DiagramKind::Component, vec![
            Shape::new(1, ShapeKind::ProvidedInterface, 0.0, 0.0, 20.0, 20.0),
            Shape::new(2, ShapeKind::RequiredInterface, 0.0, 0.0, 20.0, 20.0),
            Shape::new(3, ShapeKind::ProvidedInterface, 0.0, 0.0, 20.0, 20.0),
            Shape::new(4, ShapeKind::RequiredInterface, 50.0, 0.0, 20.0, 20.0),
        ]);
        assert_eq!(joined_interfaces(&d, &d.shapes[0]), vec![2]);
        assert_eq!(joined_interfaces(&d, &d.shapes[1]), vec![1, 3]);
        assert!(joined_interfaces(&d, &d.shapes[3]).is_empty());
        assert!(opposite_interface(ShapeKind::ProvidedInterface, ShapeKind::RequiredInterface));
        assert!(!opposite_interface(ShapeKind::ProvidedInterface, ShapeKind::ProvidedInterface));
        assert!(!opposite_interface(ShapeKind::Class, ShapeKind::RequiredInterface));
    }

    #[test]
    fn press_lane_runs_straight_across_away_from_the_middle() {
        let a = Shape::new(1, ShapeKind::Class, 0.0, 0.0, 100.0, 60.0);
        let b = Shape::new(2, ShapeKind::Class, 200.0, 0.0, 100.0, 60.0);
        assert_eq!(press_lane(&a, &b, (50.0, 30.0)), None);
        let (fa, fb) = press_lane(&a, &b, (50.0, 12.0)).unwrap();
        assert_eq!(fa, (1.0, 0.2));
        assert_eq!(fb, (0.0, 0.2));
        let (fa, fb) = press_lane(&b, &a, (250.0, 12.0)).unwrap();
        assert_eq!((fa.0, fb.0), (0.0, 1.0));
        let u = Shape::new(3, ShapeKind::UseCase, 200.0, 0.0, 100.0, 60.0);
        assert_eq!(press_lane(&a, &u, (50.0, 12.0)), None);
    }

    #[test]
    fn set_divider_keeps_lines_apart() {
        let mut s = Shape::new(1, ShapeKind::AltFragment, 0.0, 100.0, 300.0, 200.0);
        s.dividers = Some(vec![Divider { y: 80.0, guard: String::new() }, Divider { y: 140.0, guard: String::new() }]);
        set_divider(&mut s, 0, 110.0);
        assert_eq!(s.alt_dividers()[0].y, ALT_HEAD);
        set_divider(&mut s, 0, 300.0);
        assert_eq!(s.alt_dividers()[0].y, 120.0);
        set_divider(&mut s, 1, 1000.0);
        assert_eq!(s.alt_dividers()[1].y, 190.0);
        set_divider(&mut s, 5, 150.0);
        assert_eq!(s.alt_dividers().len(), 2);
    }

    #[test]
    fn shift_lane_contents_moves_only_what_is_beyond() {
        let lane = Shape::new(1, ShapeKind::VerticalSwimlane, 0.0, 0.0, 300.0, 200.0);
        let mut d = diagram(DiagramKind::Activity, vec![
            lane.clone(),
            Shape::new(2, ShapeKind::Action, 30.0, 30.0, 40.0, 40.0),
            Shape::new(3, ShapeKind::Action, 230.0, 30.0, 40.0, 40.0),
            Shape::new(4, ShapeKind::Action, 530.0, 30.0, 40.0, 40.0),
        ]);
        shift_lane_contents(&mut d, &lane, 150.0, 40.0);
        let xs: Vec<f32> = d.shapes.iter().map(|s| s.x).collect();
        assert_eq!(xs, vec![0.0, 30.0, 270.0, 530.0]);
    }
}
