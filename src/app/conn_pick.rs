//! Connect and Create: the menu that appears when a line is released on empty space.

use super::*;

/// How close (diagram units, sideways) to its own lifeline a message must be dropped to become
/// a self message; farther away on empty space it makes a new lifeline.
pub(super) const SELF_REACH: f32 = 50.0;

/// A line released on empty space, waiting for the person to pick what to create there.
#[derive(Clone, Copy)]
pub(super) struct ConnPick {
    pub(super) conn: ConnKind,
    pub(super) from: u64,
    /// The release point, in diagram units.
    pub(super) at: (f32, f32),
    /// The highlighted row of the menu.
    pub(super) sel: usize,
}

/// Shapes a Network diagram connection can start Connect and Create from: devices and the cloud.
pub(super) fn net_pick_source(kind: ShapeKind) -> bool {
    kind.is_network_device() || kind == ShapeKind::Cloud
}

/// Containers whose empty area a line can be released on to Connect and Create (the new shape
/// lands inside): swimlanes, network zones and a use case diagram's System.
pub(super) fn counts_as_empty(kind: ShapeKind) -> bool {
    kind.is_swimlane() || matches!(kind, ShapeKind::NetworkZone | ShapeKind::System)
}

/// Whether a line of this kind from a shape of this kind starts Connect and Create, and the shape
/// its menu suggests first: twin_on_drop's choice, or in Network diagrams the source's own type.
pub(super) fn pick_default(diagram: DiagramKind, conn: ConnKind, shape: ShapeKind) -> Option<ShapeKind> {
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
pub(super) fn pick_choices(diagram: DiagramKind, _conn: ConnKind, source: ShapeKind) -> Vec<(ShapeKind, &'static str)> {
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
pub(super) fn twin_on_drop(diagram: DiagramKind, conn: ConnKind, shape: ShapeKind) -> Option<ShapeKind> {
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

impl UmlApp {
    /// Connect and Create: the menu of what to make where a line was released on empty space.
    /// The suggested shape (the source's own type, or what the diagram's flow usually leads to)
    /// starts highlighted; Up/Down move the highlight and Enter picks it, or a click picks a row.
    /// Picking makes it there, connected; Esc or a click elsewhere cancels (no shape, no line).
    pub(super) fn conn_picker(&mut self, ctx: &egui::Context, cam: &Cam, d: &Diagram, canvas: Rect, look: &Look) {
        let Some(pick) = self.conn_pick else { return };
        let Some(src) = d.shape(pick.from) else {
            self.conn_pick = None;
            return;
        };
        let choices = pick_choices(d.kind, pick.conn, src.kind);
        if choices.is_empty() {
            self.conn_pick = None;
            return;
        }
        let at = cam.pt(pick.at.0, pick.at.1);
        // the line waiting for its end
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("conn_pick_line")));
        let stroke = Stroke::new(1.5_f32, look.accent());
        painter.extend(egui::Shape::dashed_line(&[cam.rect(src).center(), at], stroke, 6.0, 4.0));
        painter.circle(at, 4.0, Color32::WHITE, stroke);
        // keyboard: move the highlight, Enter picks it
        let n = choices.len();
        let mut sel = pick.sel.min(n - 1);
        let (down, up, enter) = ctx.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::ArrowDown), i.consume_key(Modifiers::NONE, Key::ArrowUp), i.consume_key(Modifiers::NONE, Key::Enter)));
        if down {
            sel = (sel + 1) % n;
        }
        if up {
            sel = (sel + n - 1) % n;
        }
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        const ROW: f32 = 24.0;
        const WIDTH: f32 = 190.0;
        let height = n as f32 * ROW + 34.0;
        // the menu opens beside the point and stays inside the canvas
        let x = (at.x + 10.0).min(canvas.right() - WIDTH - 8.0).max(canvas.left() + 4.0);
        let y = (at.y + 10.0).min(canvas.bottom() - height - 8.0).max(canvas.top() + 4.0);
        let mut chosen = if enter { Some(choices[sel].0) } else { None };
        let title = self.t("Create").to_string();
        let inner = egui::Area::new(egui::Id::new("conn_pick")).fixed_pos(Pos2::new(x, y)).constrain_to(canvas.shrink(4.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(WIDTH);
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(egui::RichText::new(title).strong());
                ui.add_space(2.0);
                for (i, (kind, label)) in choices.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(WIDTH, ROW), Sense::click());
                    if resp.hovered() && ui.input(|inp| inp.pointer.delta() != Vec2::ZERO) {
                        sel = i;
                    }
                    if i == sel {
                        ui.painter().rect_filled(r, 3.0, ui.visuals().selection.bg_fill.linear_multiply(0.7));
                    }
                    let icon = Rect::from_min_size(r.min + Vec2::new(4.0, 3.0), Vec2::splat(18.0));
                    render::draw_icon(ui.painter(), icon, Tool::Shape(*kind), look, &opts);
                    ui.painter().text(Pos2::new(icon.right() + 8.0, r.center().y), Align2::LEFT_CENTER, *label, egui::FontId::proportional(12.5), ui.visuals().text_color());
                    if resp.clicked() {
                        chosen = Some(*kind);
                    }
                }
            });
        });
        if let Some(kind) = chosen {
            self.conn_pick = None;
            self.conn_to_new_shape(pick.conn, pick.from, kind, pick.at.0, pick.at.1);
            return;
        }
        if let Some(p) = self.conn_pick.as_mut() {
            p.sel = sel;
        }
        let outside = ctx.input(|i| (i.pointer.primary_pressed() || i.pointer.secondary_pressed()) && i.pointer.interact_pos().map(|p| !inner.response.rect.contains(p)).unwrap_or(false));
        if outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.conn_pick = None;
            if !self.sticky {
                self.tool = Tool::Select;
            }
        }
    }
}
