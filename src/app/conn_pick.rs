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
