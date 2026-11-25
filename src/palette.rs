//! Which shapes and connectors each diagram's toolbox offers.
//! Lists follow the original's resources/diagram_palette_tooltip/<Diagram>.xml files
//! (see MECHANICS.md), trimmed to the common UML elements.

use crate::model::{ConnKind, DiagramKind, ShapeKind};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool {
    Select,
    Shape(ShapeKind),
    Conn(ConnKind),
}

#[derive(Clone, Copy)]
pub struct PaletteEntry {
    pub tool: Tool,
    pub label: &'static str,
    pub tip: &'static str,
}

const fn s(kind: ShapeKind, label: &'static str, tip: &'static str) -> PaletteEntry {
    PaletteEntry { tool: Tool::Shape(kind), label, tip }
}
const fn c(kind: ConnKind, label: &'static str, tip: &'static str) -> PaletteEntry {
    PaletteEntry { tool: Tool::Conn(kind), label, tip }
}

const NOTE: PaletteEntry = s(ShapeKind::Note, "Note", "a shape for attaching additional description");
const PACKAGE: PaletteEntry = s(ShapeKind::Package, "Package", "a container for grouping model elements");
const TEXT: PaletteEntry = s(ShapeKind::Text, "Text", "free text shown on the diagram; line breaks are kept");
const CONSTRAINT: PaletteEntry = s(ShapeKind::Constraint, "Constraint", "condition under which a behavior will be executed");
const ANCHOR: PaletteEntry = c(ConnKind::Anchor, "Anchor", "connects a note to the element it describes");
const CONTAINMENT: PaletteEntry = c(ConnKind::Containment, "Containment", "a 'part-of' relationship between the source and target elements");
const ASSOCIATION: PaletteEntry = c(ConnKind::Association, "Association", "a relationship between two classes");
const AGGREGATION: PaletteEntry = c(ConnKind::Aggregation, "Aggregation", "a part-whole relationship in which the part lives on when the whole dies");
const COMPOSITION: PaletteEntry = c(ConnKind::Composition, "Composition", "a part-whole relationship in which the part dies with the whole");
const GENERALIZATION: PaletteEntry = c(ConnKind::Generalization, "Generalization", "one element is a specialized form of the other");
const REALIZATION: PaletteEntry = c(ConnKind::Realization, "Realization", "one element realizes the behavior the other specifies");
const DEPENDENCY: PaletteEntry = c(ConnKind::Dependency, "Dependency", "changes in one element will affect the other");
const USAGE: PaletteEntry = c(ConnKind::Usage, "Usage", "one element requires the other to work");
const LINK: PaletteEntry = c(ConnKind::Link, "Link", "an instance of an association");

/// Every diagram's toolbox: its own entries plus the Text shape, placed after the Note
/// (or before the first line tool when the diagram has no Note).
pub fn palette(kind: DiagramKind) -> Vec<PaletteEntry> {
    let mut v = palette_base(kind);
    let at = v
        .iter()
        .position(|e| e.tool == Tool::Shape(ShapeKind::Note))
        .map(|i| i + 1)
        .or_else(|| v.iter().position(|e| matches!(e.tool, Tool::Conn(_))))
        .unwrap_or(v.len());
    v.insert(at, TEXT);
    v
}

fn palette_base(kind: DiagramKind) -> Vec<PaletteEntry> {
    use ShapeKind as K;
    match kind {
        DiagramKind::Class => vec![
            s(K::Class, "Class", "represents an entity within a system"),
            s(K::Interface, "Interface", "a collection of operations to be realized"),
            s(K::Enumeration, "Enumeration", "a list of named values (e.g. red, blue, green)"),
            s(K::Package, "Package", "a container for grouping model elements"),
            s(K::Collaboration, "Collaboration", "elements that together accomplish some functionality"),
            NOTE,
            CONSTRAINT,
            ASSOCIATION,
            AGGREGATION,
            COMPOSITION,
            GENERALIZATION,
            REALIZATION,
            DEPENDENCY,
            USAGE,
            ANCHOR,
            CONTAINMENT,
        ],
        DiagramKind::Component => vec![
            s(K::Component, "Component", "a modular, replaceable part of a system"),
            s(K::Interface, "Interface", "a set of operations a component provides or requires"),
            s(K::Port, "Port", "an interaction point of a component"),
            s(K::ProvidedInterface, "Provided Interface", "an interface the component offers (ball)"),
            s(K::RequiredInterface, "Required Interface", "an interface the component needs (socket)"),
            s(K::Instance, "Instance Specification", "an instance of a component or class"),
            PACKAGE,
            NOTE,
            CONSTRAINT,
            ASSOCIATION,
            AGGREGATION,
            COMPOSITION,
            GENERALIZATION,
            REALIZATION,
            DEPENDENCY,
            USAGE,
            LINK,
            ANCHOR,
            CONTAINMENT,
        ],
        DiagramKind::Deployment => vec![
            s(K::Node, "Node", "a computational resource"),
            s(K::Device, "Device Node", "a physical device with processing capability"),
            s(K::ExecutionEnvironment, "Execution Environment Node", "a software environment hosting artifacts"),
            s(K::Component, "Component", "a modular, replaceable part of a system"),
            s(K::Artifact, "Artifact", "a physical piece of information, such as a file"),
            s(K::DeploymentSpecification, "Deployment Specification", "parameters for deploying an artifact"),
            s(K::Interface, "Interface", "a set of operations"),
            s(K::ProvidedInterface, "Provided Interface", "an interface offered (ball)"),
            s(K::RequiredInterface, "Required Interface", "an interface needed (socket)"),
            s(K::Instance, "Instance Specification", "an instance of a node or component"),
            PACKAGE,
            NOTE,
            CONSTRAINT,
            ASSOCIATION,
            LINK,
            DEPENDENCY,
            c(ConnKind::Deployment, "Deployment", "an artifact deployed on a node"),
            c(ConnKind::Manifestation, "Manifestation", "an artifact that embodies a model element"),
            GENERALIZATION,
            REALIZATION,
            USAGE,
            ANCHOR,
            CONTAINMENT,
        ],
        DiagramKind::UseCase => vec![
            s(K::Actor, "Actor", "a role played by a user or another system"),
            s(K::UseCase, "Use Case", "a goal an actor can achieve with the system"),
            s(K::System, "System", "the boundary of the system being modeled"),
            s(K::Collaboration, "Collaboration", "elements that together accomplish some functionality"),
            NOTE,
            CONSTRAINT,
            ASSOCIATION,
            c(ConnKind::Include, "Include", "the base use case always includes the other"),
            c(ConnKind::Extend, "Extend", "the use case optionally extends the other"),
            GENERALIZATION,
            DEPENDENCY,
            REALIZATION,
            ANCHOR,
            CONTAINMENT,
        ],
        DiagramKind::Sequence => vec![
            s(K::Lifeline, "Lifeline", "an individual participant in the interaction"),
            s(K::ActorLifeline, "Actor", "an actor taking part in the interaction"),
            s(K::AltFragment, "Alternative Combined Fragment", "alternative paths chosen by guards"),
            s(K::LoopFragment, "Loop Combined Fragment", "a fragment repeated while its guard holds"),
            NOTE,
            c(ConnKind::Message, "Message", "a synchronous call from one lifeline to another"),
            c(ConnKind::AsyncMessage, "Asynchronous Message", "a signal sent without waiting"),
            c(ConnKind::ReturnMessage, "Return Message", "the reply to an earlier call"),
            c(ConnKind::CreateMessage, "Create Message", "creates the target lifeline"),
            ANCHOR,
        ],
        DiagramKind::StateMachine => vec![
            s(K::State, "State", "a condition during the life of an object"),
            s(K::Initial, "Initial Pseudo State", "where the state machine starts"),
            s(K::Final, "Final State", "where the state machine completes"),
            s(K::Choice, "Choice", "a dynamic branch chosen by guards"),
            s(K::Junction, "Junction", "joins or splits transitions"),
            s(K::Fork, "Fork", "splits into concurrent paths"),
            s(K::Join, "Join", "merges concurrent paths"),
            s(K::ShallowHistory, "Shallow History", "resumes the last direct substate"),
            s(K::DeepHistory, "Deep History", "resumes the last nested substate"),
            s(K::EntryPoint, "Entry Point", "an entry into a composite state"),
            s(K::ExitPoint, "Exit Point", "an exit from a composite state"),
            s(K::Terminate, "Terminate", "ends the state machine immediately"),
            NOTE,
            CONSTRAINT,
            c(ConnKind::Transition, "Transition", "a change from one state to another"),
            ANCHOR,
            CONTAINMENT,
        ],
        DiagramKind::Activity => vec![
            s(K::Action, "Action", "a single step in an activity"),
            s(K::Initial, "Initial Node", "where the flow starts"),
            s(K::ActivityFinal, "Activity Final Node", "stops all flows of the activity"),
            s(K::FlowFinal, "Flow Final Node", "ends one flow"),
            s(K::Decision, "Decision Node", "chooses one of several outgoing flows"),
            s(K::Merge, "Merge Node", "brings alternative flows together"),
            s(K::Fork, "Fork Node", "splits a flow into concurrent flows"),
            s(K::Join, "Join Node", "synchronizes concurrent flows"),
            s(K::ObjectNode, "Object Node", "an object flowing through the activity"),
            s(K::SendSignal, "Send Signal Action", "sends a signal"),
            s(K::AcceptEvent, "Accept Event Action", "waits for an event"),
            s(K::AcceptTimeEvent, "Accept Time Event Action", "waits for a time event"),
            s(K::VerticalSwimlane, "Vertical Swimlane", "a partition of responsibility"),
            s(K::HorizontalSwimlane, "Horizontal Swimlane", "a partition of responsibility"),
            NOTE,
            CONSTRAINT,
            c(ConnKind::ControlFlow, "Control Flow", "passes control to the next node"),
            c(ConnKind::ObjectFlow, "Object Flow", "passes an object to the next node"),
            ANCHOR,
            CONTAINMENT,
        ],
        DiagramKind::Object => vec![
            s(K::Instance, "Instance Specification", "an object: an instance of a class"),
            s(K::Class, "Class", "represents an entity within a system"),
            PACKAGE,
            NOTE,
            CONSTRAINT,
            LINK,
            ASSOCIATION,
            AGGREGATION,
            COMPOSITION,
            DEPENDENCY,
            GENERALIZATION,
            REALIZATION,
            ANCHOR,
            CONTAINMENT,
        ],
        DiagramKind::Package => vec![
            s(K::Package, "Package", "a container for grouping model elements"),
            s(K::Subsystem, "Subsystem", "a unit of hierarchical decomposition"),
            NOTE,
            CONSTRAINT,
            DEPENDENCY,
            c(ConnKind::Import, "Import", "a package imports the members of another"),
            c(ConnKind::Access, "Access", "an import with private visibility"),
            c(ConnKind::Merge, "Merge", "combines the contents of two packages"),
            GENERALIZATION,
            REALIZATION,
            ANCHOR,
            CONTAINMENT,
        ],
        DiagramKind::Network => vec![
            s(K::Cloud, "Internet / Cloud", "the internet or another outside network"),
            s(K::Router, "Router", "forwards traffic between networks"),
            s(K::Switch, "Switch", "connects the devices of one network"),
            s(K::Hub, "Hub", "repeats traffic to every port"),
            s(K::Firewall, "Firewall", "filters traffic between networks"),
            s(K::WebFirewall, "Web Firewall (WAF)", "filters web (HTTP) traffic in front of web servers"),
            s(K::LoadBalancer, "Load Balancer", "spreads requests over several servers"),
            s(K::Server, "Server", "a server computer"),
            s(K::Database, "Database", "a database server"),
            s(K::Storage, "Storage", "a storage system (disk array, SAN, NAS)"),
            s(K::Pc, "PC", "a desktop computer"),
            s(K::Laptop, "Laptop", "a notebook computer"),
            s(K::Phone, "Phone", "a mobile phone or IP phone"),
            s(K::AccessPoint, "Wireless AP", "a wireless access point"),
            s(K::Person, "Person", "a user or administrator"),
            s(K::NetworkZone, "Zone", "a network segment (LAN, DMZ, VLAN...) holding devices"),
            NOTE,
            c(ConnKind::Link, "Connection", "a wired network connection"),
            c(ConnKind::Wireless, "Wireless Connection", "a wireless connection"),
            ANCHOR,
        ],
        DiagramKind::MindMap => vec![
            s(K::CentralTopic, "Central Topic", "the main idea the mind map grows from"),
            s(K::Topic, "Topic", "an idea branching off another topic (select a topic: Tab adds a sub-topic, Enter a sibling)"),
            NOTE,
            // no Branch tool: branches come from Tab/Enter and from dragging a topic onto another
            ANCHOR,
        ],
        DiagramKind::Timeline => vec![
            s(K::TimeAxis, "Time Axis", "the time line: its tick labels (one per line), direction (across or down) and look are set in its properties"),
            s(K::TimeEvent, "Event", "a label whose line drops to the time axis at its place (a flag, a dot, a pin, a milestone diamond or none)"),
            NOTE,
            ANCHOR,
        ],
        DiagramKind::Erd => vec![
            s(K::Entity, "Entity", "a database table"),
            s(K::DbView, "View", "a database view"),
            NOTE,
            c(ConnKind::OneToOne, "One-to-One Relationship", "one row relates to exactly one row"),
            c(ConnKind::OneToMany, "One-to-Many Relationship", "one row relates to many rows"),
            c(ConnKind::ManyToOne, "Many-to-One Relationship", "many rows relate to one row"),
            c(ConnKind::ManyToMany, "Many-to-Many Relationship", "many rows relate to many rows"),
            ANCHOR,
        ],
    }
}

/// The toolbox label for a shape or connector kind within a diagram.
pub fn label_for(kind: DiagramKind, tool: Tool) -> &'static str {
    palette(kind)
        .into_iter()
        .find(|e| e.tool == tool)
        .map(|e| e.label)
        .unwrap_or(match tool {
            Tool::Select => "Select",
            Tool::Shape(_) => "Shape",
            // mind map branches have no toolbox entry but keep their name
            Tool::Conn(ConnKind::Branch) => "Branch",
            Tool::Conn(_) => "Connector",
        })
}

/// Default name of a newly created shape (the original names them "Class", "Class2", ...).
pub fn base_name(kind: ShapeKind) -> &'static str {
    use ShapeKind::*;
    match kind {
        Class => "Class",
        Interface => "Interface",
        Enumeration => "Enumeration",
        Package => "Package",
        Subsystem => "Subsystem",
        Note => "",
        Text => "Text",
        Router => "Router",
        Switch => "Switch",
        Hub => "Hub",
        Server => "Server",
        Firewall => "Firewall",
        WebFirewall => "WAF",
        LoadBalancer => "LB",
        Pc => "PC",
        Laptop => "Laptop",
        Phone => "Phone",
        AccessPoint => "AP",
        Database => "DB",
        Person => "User",
        Storage => "Storage",
        Cloud => "Internet",
        NetworkZone => "Zone",
        CentralTopic => "Central Topic",
        Topic => "Topic",
        TimeAxis => "Timeline",
        TimeEvent => "Event",
        TimeMilestone => "Milestone",
        Constraint => "constraint",
        Collaboration => "Collaboration",
        Component => "Component",
        Port => "",
        Node => "Node",
        Device => "Device",
        ExecutionEnvironment => "Execution Environment",
        Artifact => "Artifact",
        DeploymentSpecification => "Deployment Specification",
        Instance => "Instance",
        ProvidedInterface => "IProvided",
        RequiredInterface => "IRequired",
        Actor => "Actor",
        UseCase => "Use Case",
        System => "System",
        Lifeline => "Lifeline",
        ActorLifeline => "Actor",
        AltFragment => "",
        LoopFragment => "",
        State => "State",
        Action => "Action",
        ObjectNode => "Object",
        SendSignal => "Send Signal",
        AcceptEvent => "Accept Event",
        AcceptTimeEvent => "",
        VerticalSwimlane | HorizontalSwimlane => "Swimlane",
        Entity => "Entity",
        DbView => "View",
        _ => "",
    }
}
