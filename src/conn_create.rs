//! Connect and Create (연결하며 만들기) rules: which shape a line released on empty space
//! makes, and what the menu offers. The menu itself lives in app/conn_pick.rs.

use crate::model::{ConnKind, DiagramKind, ShapeKind};
use crate::palette::{self, Tool};

/// Shapes a Network diagram connection can start Connect and Create from: devices and the cloud.
pub fn net_pick_source(kind: ShapeKind) -> bool {
    kind.is_network_device() || kind == ShapeKind::Cloud
}

/// Containers whose empty area a line can be released on to Connect and Create (the new shape
/// lands inside): swimlanes, network zones and a use case diagram's System.
pub fn counts_as_empty(kind: ShapeKind) -> bool {
    kind.is_swimlane() || matches!(kind, ShapeKind::NetworkZone | ShapeKind::System)
}

/// Whether a line of this kind from a shape of this kind starts Connect and Create, and the shape
/// its menu suggests first: twin_on_drop's choice, or in Network diagrams the source's own type.
pub fn pick_default(diagram: DiagramKind, conn: ConnKind, shape: ShapeKind) -> Option<ShapeKind> {
    if diagram == DiagramKind::Network {
        return (matches!(conn, ConnKind::Link | ConnKind::Wireless) && net_pick_source(shape)).then_some(shape);
    }
    // Use Case diagrams: an Association or Dependency from an Actor makes a Use Case
    if diagram == DiagramKind::UseCase {
        return (shape == ShapeKind::Actor && matches!(conn, ConnKind::Association | ConnKind::Dependency)).then_some(ShapeKind::UseCase);
    }
    twin_on_drop(diagram, conn, shape)
}

/// What the Connect and Create menu offers for a line in a diagram, in toolbox order: the
/// diagram's shapes a line can end on, leaving out notes, text, constraints, swimlanes,
/// fragments, network zones and ports, and an initial node or state (nothing flows into it).
/// A line from an Actor in a Use Case diagram only offers Use Case.
pub fn pick_choices(diagram: DiagramKind, _conn: ConnKind, source: ShapeKind) -> Vec<(ShapeKind, &'static str)> {
    use ShapeKind as K;
    let erd = diagram == DiagramKind::Erd;
    let actor = diagram == DiagramKind::UseCase && source == K::Actor;
    palette::palette(diagram)
        .into_iter()
        .filter_map(|e| if let Tool::Shape(k) = e.tool { Some((k, e.label)) } else { None })
        .filter(|(k, _)| {
            !matches!(k, K::Note | K::Text | K::Constraint | K::AltFragment | K::LoopFragment | K::NetworkZone | K::Port | K::Initial)
                && !k.is_swimlane()
                && (!erd || matches!(k, K::Entity | K::DbView))
                && (!actor || *k == K::UseCase)
        })
        .collect()
}

/// Connect and Create (연결하며 만들기): the shape a line of this kind, dragged from a shape of
/// this kind and released on empty space, makes there (connected from the source), or None.
/// Mostly a shape of the source's own type: in Class and Component diagrams for the
/// association family, Generalization, Realization, Dependency and Usage (and Link in
/// Component diagrams); in Deployment,
/// Object, Package and Entity Relationship diagrams for every relationship line they offer
/// (never Anchor or Containment) from their main shapes. In Activity diagrams a flow from an
/// action-like node makes the same node and one from a control node (initial, decision, merge,
/// fork, join) makes an Action; in State Machine diagrams a transition from a state or a
/// pseudo state that can lead on makes a State. Never from notes, constraints, ports, interface
/// balls and sockets or final nodes.
pub fn twin_on_drop(diagram: DiagramKind, conn: ConnKind, shape: ShapeKind) -> Option<ShapeKind> {
    use ConnKind::*;
    use ShapeKind as K;
    let plain = !shape.is_pseudo() && !matches!(shape, K::Note | K::Text | K::Constraint);
    let same = |ok: bool| (ok && plain).then_some(shape);
    match diagram {
        DiagramKind::Class => same(matches!(conn, Association | Aggregation | Composition | Generalization | Realization | Dependency | Usage)),
        DiagramKind::Component => same(matches!(conn, Association | Aggregation | Composition | Generalization | Realization | Dependency | Usage | Link)),
        DiagramKind::Deployment => same(
            matches!(conn, Association | Link | Dependency | Deployment | Manifestation | Generalization | Realization | Usage)
                && matches!(shape, K::Node | K::Device | K::ExecutionEnvironment | K::Component | K::Artifact | K::Interface | K::Instance | K::Package),
        ),
        DiagramKind::Object => same(matches!(conn, Link | Association | Aggregation | Composition | Dependency | Generalization | Realization) && matches!(shape, K::Instance | K::Class | K::Package)),
        DiagramKind::Package => same(matches!(conn, Dependency | Import | Access | Merge | Generalization | Realization) && matches!(shape, K::Package | K::Subsystem)),
        DiagramKind::Erd => same(conn.is_erd() && matches!(shape, K::Entity | K::DbView)),
        // a branch dragged off a topic makes a sub-topic
        DiagramKind::MindMap => (conn == Branch && shape.is_topic()).then_some(K::Topic),
        DiagramKind::Activity if matches!(conn, ControlFlow | ObjectFlow) => match shape {
            K::Action | K::ObjectNode | K::SendSignal | K::AcceptEvent => Some(shape),
            K::Initial | K::Decision | K::Merge | K::Fork | K::Join | K::AcceptTimeEvent => Some(K::Action),
            _ => None,
        },
        DiagramKind::StateMachine if conn == Transition => match shape {
            K::State | K::Initial | K::Choice | K::Junction | K::Fork | K::Join | K::ShallowHistory | K::DeepHistory | K::EntryPoint | K::ExitPoint => Some(K::State),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ShapeKind as K;

    #[test]
    fn twin_on_drop_rules() {
        assert_eq!(twin_on_drop(DiagramKind::Class, ConnKind::Association, K::Class), Some(K::Class));
        assert_eq!(twin_on_drop(DiagramKind::Class, ConnKind::Association, K::Note), None);
        assert_eq!(twin_on_drop(DiagramKind::Class, ConnKind::Anchor, K::Class), None);
        assert_eq!(twin_on_drop(DiagramKind::Activity, ConnKind::ControlFlow, K::Action), Some(K::Action));
        assert_eq!(twin_on_drop(DiagramKind::Activity, ConnKind::ControlFlow, K::Decision), Some(K::Action));
        assert_eq!(twin_on_drop(DiagramKind::StateMachine, ConnKind::Transition, K::Initial), Some(K::State));
        assert_eq!(twin_on_drop(DiagramKind::Sequence, ConnKind::Message, K::Lifeline), None);
    }

    #[test]
    fn pick_default_rules() {
        assert_eq!(pick_default(DiagramKind::Network, ConnKind::Link, K::Cloud), Some(K::Cloud));
        assert_eq!(pick_default(DiagramKind::Network, ConnKind::Link, K::NetworkZone), None);
        assert_eq!(pick_default(DiagramKind::MindMap, ConnKind::Branch, K::CentralTopic), Some(K::Topic));
        assert_eq!(pick_default(DiagramKind::MindMap, ConnKind::Branch, K::Topic), Some(K::Topic));
        assert_eq!(pick_default(DiagramKind::MindMap, ConnKind::Branch, K::Note), None);
        assert_eq!(pick_default(DiagramKind::UseCase, ConnKind::Association, K::Actor), Some(K::UseCase));
        assert_eq!(pick_default(DiagramKind::UseCase, ConnKind::Association, K::UseCase), None);
        assert_eq!(pick_default(DiagramKind::Class, ConnKind::Generalization, K::Class), Some(K::Class));
    }

    #[test]
    fn pick_choices_rules() {
        let erd: Vec<ShapeKind> = pick_choices(DiagramKind::Erd, ConnKind::OneToMany, K::Entity).into_iter().map(|(k, _)| k).collect();
        assert!(!erd.is_empty() && erd.iter().all(|k| matches!(k, K::Entity | K::DbView)));
        let actor: Vec<ShapeKind> = pick_choices(DiagramKind::UseCase, ConnKind::Association, K::Actor).into_iter().map(|(k, _)| k).collect();
        assert_eq!(actor, vec![K::UseCase]);
        let act: Vec<ShapeKind> = pick_choices(DiagramKind::Activity, ConnKind::ControlFlow, K::Action).into_iter().map(|(k, _)| k).collect();
        assert!(act.contains(&K::Action));
        assert!(act.iter().all(|k| !matches!(k, K::Note | K::Initial) && !k.is_swimlane()));
    }
}
