//! The canvas: drawing, hit testing, handles, the cursor and the right-click menu.

use super::*;

/// The line between two partitions of a swimlane under a screen point: (swimlane, index of the
/// partition before the line).
pub(super) fn partition_edge_at(d: &Diagram, cam: &Cam, p: Pos2) -> Option<(u64, usize)> {
    for s in d.shapes.iter().rev().filter(|s| s.kind.is_swimlane()) {
        let r = cam.rect(s);
        if !r.expand(3.0).contains(p) {
            continue;
        }
        let vertical = s.kind == ShapeKind::VerticalSwimlane;
        for (i, (at, _)) in s.partition_spans().iter().enumerate().skip(1) {
            let hit = if vertical { (cam.pt(s.x + at, 0.0).x - p.x).abs() <= 5.0 } else { (cam.pt(0.0, s.y + at).y - p.y).abs() <= 5.0 };
            if hit {
                return Some((s.id, i - 1));
            }
        }
    }
    None
}

/// The section line of an alternative fragment under a screen point: (fragment, line index,
/// its height in diagram units).
pub(super) fn divider_at(d: &Diagram, cam: &Cam, p: Pos2) -> Option<(u64, usize, f32)> {
    for s in d.shapes.iter().rev().filter(|s| s.kind == ShapeKind::AltFragment) {
        let r = cam.rect(s);
        if p.x < r.left() || p.x > r.right() {
            continue;
        }
        for (i, dv) in s.alt_dividers().iter().enumerate() {
            let y = s.y + dv.y.clamp(crate::model::ALT_HEAD.min(s.h), s.h.max(0.0));
            if (cam.pt(0.0, y).y - p.y).abs() <= 5.0 {
                return Some((s.id, i, dv.y));
            }
        }
    }
    None
}

impl UmlApp {
    /// The selected activation bar on screen, with its top and bottom handle points.
    pub(super) fn bar_handles(&self, d: &Diagram, cam: &Cam) -> Option<(Rect, Pos2, Pos2)> {
        let (lid, depth, ymid) = self.sel_bar?;
        if d.kind != DiagramKind::Sequence {
            return None;
        }
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        if !opts.show_activations {
            return None;
        }
        let lay = crate::seq::layout(d, &opts);
        let bar = lay.bars.iter().filter(|b| b.lifeline == lid && b.depth == depth).find(|b| ymid >= b.y0 - 0.5 && ymid <= b.y1 + 0.5)?;
        let s = d.shape(lid)?;
        let cx = s.x + s.w / 2.0 + bar.depth as f32 * crate::seq::BAR_STEP;
        let r = Rect::from_min_max(cam.pt(cx - crate::seq::BAR_HALF, bar.y0), cam.pt(cx + crate::seq::BAR_HALF, bar.y1));
        Some((r, Pos2::new(r.center().x, r.top()), Pos2::new(r.center().x, r.bottom())))
    }

    /// A click on an activation bar (not on a message) selects that bar.
    pub(super) fn bar_click(&mut self, d: &Diagram, cam: &Cam, p: Pos2) -> bool {
        if d.kind != DiagramKind::Sequence || matches!(self.hit_any(cam, p), Some(EditTarget::Conn(_))) || self.label_at(p).is_some() {
            return false;
        }
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let Some(b) = bar_at(d, &opts, cam.zoom, cam.to_diagram(p)) else { return false };
        self.clear_selection();
        self.sel_bar = Some((b.lifeline, b.depth, (b.y0 + b.y1) / 2.0));
        true
    }

    /// 2 = strong hit (body), 1 = weak (a lifeline's dashed line), 0 = none
    pub(super) fn shape_hit(cam: &Cam, s: &Shape, p: Pos2) -> u8 {
        let r = cam.rect(s);
        if s.kind.is_lifeline() {
            let head_h = if s.kind == ShapeKind::Lifeline { 40.0 } else { 70.0 } * cam.zoom;
            let head = Rect::from_min_size(r.min, Vec2::new(r.width(), head_h));
            if head.expand(2.0).contains(p) {
                return 2;
            }
            if (p.x - r.center().x).abs() < 7.0 && p.y >= r.top() && p.y <= r.bottom() {
                return 1;
            }
            return 0;
        }
        let rr = if s.kind.is_pseudo() || s.kind.fixed_size() { r.expand(4.0) } else { r.expand(1.0) };
        if rr.contains(p) {
            2
        } else {
            0
        }
    }

    /// Whether a shape that isn't a container (an action, a node, a note...) is under a screen
    /// point: a swimlane's or fragment's own lines and headers are only reachable where none is.
    pub(super) fn over_plain_shape(d: &Diagram, cam: &Cam, p: Pos2) -> bool {
        d.shapes.iter().any(|s| !s.kind.is_container() && Self::shape_hit(cam, s, p) == 2)
    }

    pub(super) fn hit_shape(&self, d: &Diagram, cam: &Cam, p: Pos2, allow_weak: bool) -> Option<u64> {
        for s in d.shapes.iter().rev().filter(|s| !s.kind.is_container()) {
            let h = Self::shape_hit(cam, s, p);
            if h == 2 || (allow_weak && h == 1) {
                return Some(s.id);
            }
        }
        let mut containers: Vec<&Shape> = d.shapes.iter().filter(|s| s.kind.is_container() && Self::shape_hit(cam, s, p) > 0).collect();
        containers.sort_by(|a, b| (a.w * a.h).partial_cmp(&(b.w * b.h)).unwrap_or(std::cmp::Ordering::Equal));
        containers.first().map(|s| s.id)
    }

    pub(super) fn hit_conn(&self, d: &Diagram, cam: &Cam, p: Pos2) -> Option<u64> {
        let mut best: Option<(u64, f32)> = None;
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let (_, paths) = crate::seq::all_paths(d, &opts);
        for c in &d.connectors {
            let Some(dpath) = paths.get(&c.id) else { continue };
            let path: Vec<Pos2> = dpath.iter().map(|(x, y)| cam.pt(*x, *y)).collect();
            let dist = render::distance_to_path(p, &path);
            if dist < 6.0 && best.map(|(_, bd)| dist < bd).unwrap_or(true) {
                best = Some((c.id, dist));
            }
        }
        best.map(|(id, _)| id)
    }

    /// Strong shape, then connector, then weak shape/containers.
    pub(super) fn hit_any(&self, cam: &Cam, p: Pos2) -> Option<EditTarget> {
        let d = self.diag()?;
        for s in d.shapes.iter().rev().filter(|s| !s.kind.is_container()) {
            if Self::shape_hit(cam, s, p) == 2 {
                return Some(EditTarget::Shape(s.id));
            }
        }
        if let Some(c) = self.hit_conn(d, cam, p) {
            return Some(EditTarget::Conn(c));
        }
        self.hit_shape(d, cam, p, true).map(EditTarget::Shape)
    }

    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        self.canvas_rect = rect;
        let look = self.look();
        painter.rect_filled(rect, 0.0, look.background());
        if self.fit_request {
            self.fit_request = false;
            self.fit_to_window();
        }
        // the first time a diagram is shown, make sure all of it is visible
        if let Some(id) = self.active {
            if !self.views.contains_key(&id) {
                self.view_mut();
                self.fit_view(1.0, true);
            }
        }
        if self.project.is_none() {
            painter.text(rect.center(), Align2::CENTER_CENTER, self.t("No diagram is open."), egui::FontId::proportional(16.0), ui.visuals().weak_text_color());
            return;
        }
        let opts = self.project.as_ref().unwrap().options.clone();
        let view = self.view();
        let cam = Cam { origin: rect.min, pan: view.pan, zoom: view.zoom };

        // grid
        if view.show_grid && self.shot.is_none() && self.active.is_some() {
            let step = self.settings.grid_size * cam.zoom;
            if step >= 5.0 {
                // GridColor default is Color.LIGHT_GRAY (v.ddg.r.<clinit>, field bp)
                let gc = if look.dark { Color32::from_rgb(70, 72, 78) } else { Color32::from_rgb(192, 192, 192) };
                let major = gc;
                let base_x = rect.min.x + cam.pan.x;
                let base_y = rect.min.y + cam.pan.y;
                let mut i = ((rect.min.x - base_x) / step).floor() as i64;
                loop {
                    let x = base_x + i as f32 * step;
                    if x > rect.max.x {
                        break;
                    }
                    let c = if i.rem_euclid(5) == 0 { major } else { gc };
                    painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], Stroke::new(1.0_f32, c));
                    i += 1;
                }
                let mut j = ((rect.min.y - base_y) / step).floor() as i64;
                loop {
                    let y = base_y + j as f32 * step;
                    if y > rect.max.y {
                        break;
                    }
                    let c = if j.rem_euclid(5) == 0 { major } else { gc };
                    painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)], Stroke::new(1.0_f32, c));
                    j += 1;
                }
            }
        }

        // grow shapes to fit their text
        if opts.auto_fit_shapes {
            let ctx = ui.ctx().clone();
            if let Some(d) = self.diag_mut() {
                for s in d.shapes.iter_mut() {
                    let m = render::min_size(&ctx, s, &opts, &look);
                    if s.w < m.x {
                        s.w = m.x;
                    }
                    if s.h < m.y {
                        s.h = m.y;
                    }
                }
            }
        }

        // a mind map places its topics by its structure (not while they are being dragged)
        if !matches!(self.drag, Drag::Move { .. }) {
            if let Some(d) = self.diag_mut() {
                crate::mindlayout::layout(d);
            }
        }

        let Some(d) = self.diag().cloned() else {
            painter.text(rect.center(), Align2::CENTER_CENTER, self.t("No diagram is open."), egui::FontId::proportional(16.0), ui.visuals().weak_text_color());
            painter.text(
                rect.center() + Vec2::new(0.0, 24.0),
                Align2::CENTER_CENTER,
                self.t("Create a diagram from the Diagram menu or the Diagram Navigator."),
                egui::FontId::proportional(13.0),
                ui.visuals().weak_text_color(),
            );
            return;
        };

        let painter = painter.with_clip_rect(rect);
        // containers (largest first), then other shapes, then connectors
        let mut containers: Vec<&Shape> = d.shapes.iter().filter(|s| s.kind.is_container()).collect();
        containers.sort_by(|a, b| (b.w * b.h).partial_cmp(&(a.w * a.h)).unwrap_or(std::cmp::Ordering::Equal));
        for s in containers {
            render::draw_shape(&painter, &cam, s, &look, &opts);
        }
        for s in d.shapes.iter().filter(|s| !s.kind.is_container()) {
            render::draw_shape(&painter, &cam, s, &look, &opts);
        }
        // message numbers follow their vertical order
        // activation bars, message numbers and every line's path
        let (lay, paths) = crate::seq::all_paths(&d, &opts);
        if opts.show_activations {
            for bar in &lay.bars {
                if let Some(l) = d.shape(bar.lifeline) {
                    render::draw_activation(&painter, &cam, bar, l, &look);
                }
            }
        }
        // names drawn beside small shapes can be dragged
        self.label_rects.clear();
        for s in &d.shapes {
            if let Some(r) = render::name_below_rect(&painter, &cam, s, &look) {
                self.label_rects.push((LabelTarget::Shape(s.id), r));
            }
        }
        // where lines cross, one jumps over the other
        let screen: Vec<(u64, Vec<Pos2>)> = d.connectors.iter().filter_map(|c| paths.get(&c.id).map(|p| (c.id, p.iter().map(|(x, y)| cam.pt(*x, *y)).collect()))).collect();
        let hops = render::crossings(&screen);
        for c in &d.connectors {
            let Some(path) = paths.get(&c.id) else { continue };
            let Some(a) = d.shape(c.from) else { continue };
            let b = d.shape(c.to).unwrap_or(a);
            let h = hops.get(&c.id).map(|v| v.as_slice()).unwrap_or(&[]);
            let [name, from, to] = render::draw_connector(&painter, &cam, d.kind, c, a, b, &look, &opts, lay.numbers.get(&c.id).map(|s| s.as_str()), path, h, self.sel_conns.contains(&c.id));
            for (t, r) in [(LabelTarget::Conn(c.id), name), (LabelTarget::ConnFrom(c.id), from), (LabelTarget::ConnTo(c.id), to)] {
                if let Some(r) = r {
                    self.label_rects.push((t, r));
                }
            }
        }
        for s in &d.shapes {
            render::device_name_over_lines(&painter, &cam, s, &look);
        }
        for id in &self.sel_shapes {
            if let Some(s) = d.shape(*id) {
                render::draw_selection(&painter, &cam, s, &look);
            }
        }
        // the selected activation: outlined, with a handle at its top and bottom
        if let Some((r, top, bottom)) = self.bar_handles(&d, &cam) {
            painter.rect_stroke(r.expand(2.0), 0.0, Stroke::new(1.5_f32, look.accent()), StrokeKind::Middle);
            for h in [top, bottom] {
                painter.rect(Rect::from_center_size(h, Vec2::splat(7.0)), 0.0, Color32::WHITE, Stroke::new(1.2_f32, look.accent()), StrokeKind::Middle);
            }
        }
        // bend points (squares) and segment middles (dots) of the selected line
        if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
            if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                // its two ends (diamonds) can slide along the shapes' outlines
                if let Some(ends) = self.end_handles(&d, &cam, c) {
                    for (k, e) in ends.iter().enumerate() {
                        let fixed = if k == 0 { c.from_anchor.is_some() } else { c.to_anchor.is_some() };
                        let pts = vec![*e + Vec2::new(0.0, -5.0), *e + Vec2::new(5.0, 0.0), *e + Vec2::new(0.0, 5.0), *e + Vec2::new(-5.0, 0.0)];
                        let fill = if fixed { look.accent() } else { Color32::WHITE };
                        painter.add(egui::epaint::Shape::convex_polygon(pts, fill, Stroke::new(1.2_f32, look.accent())));
                    }
                }
                if let Some((points, mids)) = Self::point_handles(&d, &cam, c) {
                    for m in mids {
                        painter.circle(m, 3.5, Color32::WHITE, Stroke::new(1.0_f32, look.accent()));
                    }
                    for p in points {
                        painter.rect(Rect::from_center_size(p, Vec2::splat(7.0)), 0.0, look.accent(), Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Middle);
                    }
                }
            }
        }

        if self.conn_pick.is_none() {
            self.canvas_input(ui, &resp, &cam, &d);
        }

        // previews
        match &self.drag {
            Drag::Move { .. } if d.kind == DiagramKind::MindMap => self.mind_drop_preview(&painter, &cam, &d, &look),
            Drag::Rubber { start, cur } => {
                let r = Rect::from_two_pos(*start, *cur);
                painter.rect(r, 0.0, look.accent().linear_multiply(0.08), Stroke::new(1.0_f32, look.accent()), StrokeKind::Middle);
            }
            Drag::Connect { from, cur, start: press } => {
                if let Some(s) = d.shape(*from) {
                    let r = cam.rect(s);
                    let seq_msg = d.kind == DiagramKind::Sequence && s.kind.is_lifeline() && matches!(self.tool, Tool::Conn(ConnKind::Message | ConnKind::AsyncMessage));
                    let over_other = self.hit_shape(&d, &cam, *cur, true).map(|id| id != *from && d.shape(id).map(|o| o.kind.is_lifeline()).unwrap_or(false)).unwrap_or(false);
                    if seq_msg && !over_other && (cur.x - r.center().x).abs() < SELF_REACH * cam.zoom {
                        // back near its own lifeline: it will be a self message
                        let y = cam.pt(0.0, press.1).y;
                        let out = r.center().x + 40.0 * cam.zoom;
                        let drop = crate::seq::SELF_DROP * cam.zoom;
                        let pts = vec![Pos2::new(r.center().x, y), Pos2::new(out, y), Pos2::new(out, y + drop), Pos2::new(r.center().x, y + drop)];
                        painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, look.accent())));
                    } else {
                        let start = if d.kind == DiagramKind::Sequence && s.kind.is_lifeline() { Pos2::new(r.center().x, cur.y) } else { r.center() };
                        painter.line_segment([start, *cur], Stroke::new(1.5_f32, look.accent()));
                        // Connect and Create: over empty space a "+" in a ring marks where the
                        // shape picked from the menu will go
                        if let Tool::Conn(ck) = self.tool {
                            if pick_default(d.kind, ck, s.kind).is_some() && self.hit_shape(&d, &cam, *cur, true).map(|id| d.shape(id).map(|o| counts_as_empty(o.kind)).unwrap_or(false)).unwrap_or(true) {
                                let st = Stroke::new(1.2_f32, look.accent());
                                let ring: Vec<Pos2> = (0..=24).map(|i| *cur + Vec2::angled(i as f32 / 24.0 * std::f32::consts::TAU) * 16.0).collect();
                                for seg in ring.chunks(2) {
                                    if seg.len() == 2 {
                                        painter.line_segment([seg[0], seg[1]], st);
                                    }
                                }
                                painter.line_segment([*cur - Vec2::new(6.0, 0.0), *cur + Vec2::new(6.0, 0.0)], st);
                                painter.line_segment([*cur - Vec2::new(0.0, 6.0), *cur + Vec2::new(0.0, 6.0)], st);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        if let Tool::Shape(_) | Tool::Conn(_) = self.tool {
            let label = palette::label_for(d.kind, self.tool);
            painter.text(rect.left_bottom() + Vec2::new(8.0, -8.0), Align2::LEFT_BOTTOM, format!("✚ {label}"), egui::FontId::proportional(12.0), look.accent());
        }

        self.conn_picker(ui.ctx(), &cam, &d, rect, &look);
        self.inline_editor(ui.ctx(), &cam);
    }

    /// The mouse pointer tells what a press would do: diagonal arrows on resize handles,
    /// a hand on movable names and bend points, a cross while drawing.
    pub(super) fn canvas_cursor(&self, ctx: &egui::Context, resp: &egui::Response, cam: &Cam, d: &Diagram, pointer: Option<Pos2>) {
        use egui::CursorIcon as C;
        if !(resp.hovered() || resp.dragged()) {
            return;
        }
        let corner_icon = |i: usize| if i % 2 == 0 { C::ResizeNwSe } else { C::ResizeNeSw };
        let dragging = match &self.drag {
            Drag::Resize { corner, .. } => Some(corner_icon(*corner)),
            Drag::MoveLabel { .. } | Drag::MovePoint { .. } | Drag::MoveEnd { .. } => Some(C::Grabbing),
            Drag::Move { .. } | Drag::MoveMessage { .. } | Drag::MoveLine { .. } => Some(C::Move),
            Drag::Connect { .. } => Some(C::Crosshair),
            Drag::BarEdge { .. } | Drag::Divider { .. } => Some(C::ResizeVertical),
            Drag::PartEdge { vertical, .. } => Some(if *vertical { C::ResizeHorizontal } else { C::ResizeVertical }),
            _ => None,
        };
        let icon = dragging.or_else(|| {
            let p = pointer?;
            if let Tool::Shape(_) | Tool::Conn(_) = self.tool {
                return Some(C::Crosshair);
            }
            if let Some((_, top, bottom)) = self.bar_handles(d, cam) {
                if (p - top).length() < 8.0 || (p - bottom).length() < 8.0 {
                    return Some(C::ResizeVertical);
                }
            }
            if self.tool == Tool::Select && divider_at(d, cam, p).is_some() {
                return Some(C::ResizeVertical);
            }
            if self.tool == Tool::Select && !Self::over_plain_shape(d, cam, p) {
                if let Some((id, _)) = partition_edge_at(d, cam, p) {
                    let vertical = d.shape(id).map(|s| s.kind == ShapeKind::VerticalSwimlane).unwrap_or(true);
                    return Some(if vertical { C::ResizeHorizontal } else { C::ResizeVertical });
                }
            }
            if self.sel_shapes.len() == 1 {
                if let Some(s) = d.shape(self.sel_shapes[0]) {
                    if !s.kind.fixed_size() {
                        if let Some(i) = render::handle_points(cam, s).iter().position(|h| (p - *h).length() < 7.0) {
                            return Some(corner_icon(i));
                        }
                    }
                }
            }
            if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
                if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                    if self.end_handles(d, cam, c).map(|e| e.iter().any(|q| (*q - p).length() < 8.0)).unwrap_or(false) || Self::point_handle_hit(d, cam, c, p).is_some() {
                        return Some(C::Grab);
                    }
                }
            }
            if self.label_at(p).is_some() {
                return Some(C::Grab);
            }
            None
        });
        if let Some(i) = icon {
            ctx.set_cursor_icon(i);
        }
    }

    pub(super) fn canvas_menu(&mut self, ui: &mut egui::Ui) {
        // a swimlane: add a partition after the one under the pointer, or take that one away
        if let Some((id, along)) = self.lane_menu {
            let (idx, count) = self.diag().and_then(|d| d.shape(id)).map(|s| (s.partition_at(along), s.lane_partitions().len())).unwrap_or((0, 0));
            if self.menu_item(ui, "Add Partition", None, true) {
                if let Some(i) = self.add_partition(id, idx) {
                    self.start_part_edit(id, i, true);
                }
                self.lane_menu = None;
            }
            if self.menu_item(ui, "Remove Partition", None, count > 1) {
                self.remove_partition(id, idx);
                self.lane_menu = None;
            }
            ui.separator();
        }
        // an alternative fragment: add an else section, or take the one under the pointer away
        if let Some((id, at)) = self.alt_menu {
            let count = self.diag().and_then(|d| d.shape(id)).map(|s| s.alt_dividers().len()).unwrap_or(0);
            if self.menu_item(ui, "Add Else Section", None, true) {
                if let Some(i) = self.add_alt_section(id) {
                    self.start_guard_edit(id, i);
                }
                self.alt_menu = None;
            }
            if self.menu_item(ui, "Remove Else Section", None, count > 0) {
                self.remove_alt_section(id, Some(at));
                self.alt_menu = None;
            }
            ui.separator();
        }
        // a lifeline's activation: split it here, or join it again at a split point
        if let Some((lid, y, on_bar, near)) = self.seq_menu {
            if on_bar && self.menu_item(ui, "Split Activation", None, true) {
                self.split_activation(lid, y);
                self.seq_menu = None;
            }
            if let Some(ys) = near {
                if self.menu_item(ui, "Join Activations", None, true) {
                    self.join_activations(lid, ys);
                    self.seq_menu = None;
                }
            }
            if on_bar || near.is_some() {
                ui.separator();
            }
        }
        let one_conn = if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
            self.diag().and_then(|d| d.connectors.iter().find(|c| c.id == self.sel_conns[0]).map(|c| (c.clone(), d.kind)))
        } else {
            None
        };
        if let Some((c, dkind)) = one_conn {
            let lang = self.settings.language;
            if self.menu_item(ui, "Rename", Some(&sc(Modifiers::NONE, Key::F2)), true) {
                self.start_edit(Some(EditTarget::Conn(c.id)));
            }
            if navigable(c.kind) {
                let mut end = render::shows_arrow(&c, true);
                if ui.checkbox(&mut end, tr(lang, "Arrow at target end")).changed() {
                    self.set_arrows(c.id, Some(end), None);
                    ui.close_menu();
                }
                let mut start = render::shows_arrow(&c, false);
                if ui.checkbox(&mut start, tr(lang, "Arrow at source end")).changed() {
                    self.set_arrows(c.id, None, Some(start));
                    ui.close_menu();
                }
            }
            if !(dkind == DiagramKind::Sequence && c.kind.is_message()) {
                ui.separator();
                if self.menu_item(ui, "Straight Line", None, !c.points.is_empty()) {
                    self.set_line_shape(c.id, false);
                }
                if self.menu_item(ui, "Right-Angle Line", None, true) {
                    self.set_line_shape(c.id, true);
                }
                ui.menu_button(tr(lang, "Connector Style"), |ui| {
                    for (code, name) in render::LINE_STYLES {
                        if ui.radio(c.style == code, tr(lang, name)).clicked() {
                            self.set_conn_style(c.id, code);
                            ui.close_menu();
                        }
                    }
                });
            }
            ui.separator();
        }
        let one = if self.sel_shapes.len() == 1 { self.diag().and_then(|d| d.shape(self.sel_shapes[0])).cloned() } else { None };
        if let Some(s) = &one {
            if self.menu_item(ui, "Rename", Some(&sc(Modifiers::NONE, Key::F2)), true) {
                self.start_edit(Some(EditTarget::Shape(s.id)));
            }
            if s.kind.has_attributes() {
                let label = match s.kind {
                    ShapeKind::Entity | ShapeKind::DbView => "Add Column",
                    ShapeKind::Enumeration => "Add Literal",
                    ShapeKind::Instance => "Add Slot",
                    ShapeKind::State => "Activities",
                    _ => "Add Attribute",
                };
                if self.menu_item(ui, label, None, true) {
                    self.add_line(false);
                }
            }
            if s.kind.has_operations() && self.menu_item(ui, "Add Operation", None, true) {
                self.add_line(true);
            }
            ui.separator();
        }
        let has_sel = !self.sel_shapes.is_empty();
        if has_sel {
            if self.menu_item(ui, "Cut", None, true) {
                self.cut();
            }
            if self.menu_item(ui, "Copy", None, true) {
                self.copy_selection();
            }
            if self.menu_item(ui, "Duplicate", Some(&SC_DUPLICATE), true) {
                self.duplicate();
            }
        }
        if (has_sel || !self.sel_conns.is_empty()) && self.menu_item(ui, "Delete", None, true) {
            self.delete_selection();
        }
        let any_sel = has_sel || !self.sel_conns.is_empty();
        if self.menu_item(ui, if any_sel { "Copy Selection as Image" } else { "Copy Diagram as Image" }, Some(&SC_COPY_IMAGE), self.active.is_some()) {
            self.copy_as_image();
        }
        if has_sel {
            ui.separator();
            if self.menu_item(ui, "Bring to Front", None, true) {
                self.reorder(true);
            }
            if self.menu_item(ui, "Send to Back", None, true) {
                self.reorder(false);
            }
        }
        if !has_sel && self.sel_conns.is_empty() {
            if self.menu_item(ui, "Paste", None, self.clipboard.is_some()) {
                self.paste();
            }
            if self.menu_item(ui, "Select All", None, true) {
                self.select_all();
            }
            ui.separator();
            if self.menu_item(ui, "Zoom 100% (Actual Size)", None, true) {
                self.zoom_to(1.0);
            }
            if self.menu_item(ui, "Fit to Window", None, true) {
                self.fit_to_window();
            }
        }
    }

    /// Screen positions of the two ends of a line between shapes (not sequence messages).
    pub(super) fn end_handles(&self, d: &Diagram, cam: &Cam, c: &Connector) -> Option<[Pos2; 2]> {
        if (d.kind == DiagramKind::Sequence && c.kind.is_message()) || c.from == c.to {
            return None;
        }
        d.shape(c.from)?;
        d.shape(c.to)?;
        let opts = self.project.as_ref()?.options.clone();
        let (_, paths) = crate::seq::all_paths(d, &opts);
        let p = paths.get(&c.id)?;
        let (a, b) = (p.first()?, p.last()?);
        Some([cam.pt(a.0, a.1), cam.pt(b.0, b.1)])
    }

    /// Screen positions of a line's bend points and of its segments' middle dots.
    pub(super) fn point_handles(d: &Diagram, cam: &Cam, c: &Connector) -> Option<(Vec<Pos2>, Vec<Pos2>)> {
        if d.kind == DiagramKind::Sequence && c.kind.is_message() {
            return None;
        }
        let (a, b) = (d.shape(c.from)?, d.shape(c.to)?);
        let path: Vec<Pos2> = render::conn_controls(d.kind, c, a, b).into_iter().map(|(x, y)| cam.pt(x, y)).collect();
        if path.len() < 2 || (c.points.is_empty() && c.from == c.to) {
            return None;
        }
        let points = path[1..path.len() - 1].to_vec();
        let mids = path.windows(2).map(|w| w[0] + (w[1] - w[0]) * 0.5).collect();
        Some((points, mids))
    }

    pub(super) fn point_handle_hit(d: &Diagram, cam: &Cam, c: &Connector, p: Pos2) -> Option<PointHit> {
        let (points, mids) = Self::point_handles(d, cam, c)?;
        if let Some(i) = points.iter().position(|q| (*q - p).length() < 7.0) {
            return Some(PointHit::Point(i));
        }
        mids.iter().position(|q| (*q - p).length() < 7.0).map(PointHit::Mid)
    }
}
