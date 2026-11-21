//! The left panes: Diagram Navigator (by type or folder) and the explorer.

use super::*;

/// The id Rename uses for the project itself (diagram and folder ids never reach it).
pub(super) const PROJECT_RENAME_ID: u64 = u64::MAX;

/// Folders nest at most this many levels.
pub(super) const MAX_FOLDER_DEPTH: usize = 3;

/// Something dragged in the Diagram Navigator.
#[derive(Clone, Copy, Debug)]
pub(super) enum NavDrag {
    Diagram(u64),
    Folder(u64),
}

pub(super) enum NavAction {
    NewFolder(Option<u64>),
    NewDiagram(DiagramKind, Option<u64>),
    RenameFolder(u64),
    DeleteFolder(u64),
    Move(NavDrag, Option<u64>),
}

/// One row of a left-pane tree (Diagram Navigator or Model Explorer).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum TreeKey {
    /// The project row of a tree: 0 navigator by folder, 1 navigator by type, 2 explorer.
    Root(u8),
    Folder(u64),
    /// A diagram in tree 0, 1 or 2.
    Diagram(u8, u64),
    /// A diagram-type group (UML, database, network) in the navigator by type.
    Group(u8),
    /// A diagram type in the navigator by type.
    Kind(u8),
    /// A shape (diagram, shape) in the explorer.
    Shape(u64, u64),
}

/// A visible row of a tree, as drawn last frame (the keys move through these).
#[derive(Clone, Debug)]
pub(super) struct TreeRow {
    pub key: TreeKey,
    pub depth: usize,
    pub label: String,
    pub has_children: bool,
    pub open: bool,
    /// The index of the parent row.
    pub parent: Option<usize>,
    /// The open diagram, or a selected shape.
    pub active: bool,
    pub hover: Option<String>,
}

impl TreeRow {
    fn new(key: TreeKey, depth: usize, label: String, has_children: bool, open: bool, parent: Option<usize>) -> Self {
        TreeRow { key, depth, label, has_children, open, parent, active: false, hover: None }
    }
}

/// How many folders deep a folder is (the project root is 0, a top folder 1).
pub(super) fn folder_level(p: &Project, f: Option<u64>) -> usize {
    let mut level = 0;
    let mut cur = f;
    while let Some(id) = cur {
        level += 1;
        cur = p.folder(id).and_then(|x| x.parent);
        if level > 16 {
            break;
        }
    }
    level
}

/// Levels a folder and the folders inside it take up (a folder alone is 1).
pub(super) fn folder_height(p: &Project, id: u64) -> usize {
    1 + p.folders.iter().filter(|f| f.parent == Some(id)).map(|f| folder_height(p, f.id)).max().unwrap_or(0)
}

/// Is folder `t` the folder `outer` or inside it?
pub(super) fn is_inside(p: &Project, t: u64, outer: u64) -> bool {
    let mut cur = Some(t);
    let mut n = 0;
    while let Some(id) = cur {
        if id == outer {
            return true;
        }
        cur = p.folder(id).and_then(|x| x.parent);
        n += 1;
        if n > 16 {
            break;
        }
    }
    false
}

/// Every folder with its path ("2-Design / 2.1-Behavior"), sorted by path.
pub(super) fn folder_paths(p: &Project) -> Vec<(u64, String)> {
    let mut v: Vec<(u64, String)> = p
        .folders
        .iter()
        .map(|f| {
            let mut parts = vec![f.name.clone()];
            let mut cur = f.parent;
            let mut n = 0;
            while let Some(id) = cur {
                if let Some(x) = p.folder(id) {
                    parts.push(x.name.clone());
                    cur = x.parent;
                } else {
                    break;
                }
                n += 1;
                if n > 16 {
                    break;
                }
            }
            parts.reverse();
            (f.id, parts.join(" / "))
        })
        .collect();
    v.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    v
}

impl UmlApp {
    pub(super) fn open_switcher(&mut self) {
        let items = self.switcher_items();
        let n = items.len();
        if n == 0 {
            return;
        }
        let cur = items.iter().position(|id| Some(*id) == self.active).unwrap_or(0);
        self.switcher = Some(if n > 1 { (cur + 1) % n } else { 0 });
    }

    /// Open diagrams first (in tab order), then every other diagram.
    pub(super) fn switcher_items(&self) -> Vec<u64> {
        let mut v = self.tabs.clone();
        if let Some(p) = &self.project {
            for d in &p.diagrams {
                if !v.contains(&d.id) {
                    v.push(d.id);
                }
            }
        }
        v
    }

    pub(super) fn ask_delete_diagram(&mut self, id: u64) {
        if self.settings.confirm_delete {
            self.confirm_delete = Some(id);
        } else {
            self.delete_diagram(id);
        }
    }

    pub(super) fn left_pane(&mut self, ui: &mut egui::Ui) {
        let lang = self.settings.language;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.left_tab, 0, tr(lang, "Diagram Navigator"));
            ui.selectable_value(&mut self.left_tab, 1, tr(lang, "Model Explorer"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("«").frame(false)).on_hover_text(tr(lang, "Hide this pane")).clicked() {
                    self.show_left = false;
                }
            });
        });
        ui.separator();
        // a press anywhere outside this pane takes the keyboard away from the navigator
        let pane = ui.max_rect().expand(4.0);
        if ui.ctx().input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().map(|p| !pane.contains(p)).unwrap_or(false)) {
            self.tree_focus = false;
            self.tree_last_click = None;
        }
        let Some(project) = self.project.clone() else {
            ui.label(tr(lang, "No diagram is open."));
            return;
        };
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            if self.left_tab == 0 {
                self.navigator(ui, &project);
            } else {
                self.explorer(ui, &project);
            }
        });
    }

    /// Diagram Navigator: by folder (the person's own grouping) or by diagram type.
    pub(super) fn navigator(&mut self, ui: &mut egui::Ui, project: &Project) {
        let lang = self.settings.language;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.nav_by_type, false, tr(lang, "Folders"));
            ui.selectable_value(&mut self.nav_by_type, true, tr(lang, "Diagram Types"));
        });
        ui.add_space(2.0);
        if self.nav_by_type {
            self.navigator_by_type(ui, project);
        } else {
            self.navigator_by_folder(ui, project);
        }
    }

    pub(super) fn apply_nav(&mut self, a: NavAction) {
        let lang = self.settings.language;
        let Some(p) = self.project.as_mut() else { return };
        match a {
            NavAction::NewFolder(parent) => {
                if folder_level(p, parent) >= MAX_FOLDER_DEPTH {
                    self.status = tr(lang, "Folders can be at most 3 levels deep.").to_string();
                    return;
                }
                let base = tr(lang, "New Folder").to_string();
                let mut name = base.clone();
                let mut n = 2;
                while p.folders.iter().any(|f| f.parent == parent && f.name == name) {
                    name = format!("{base} {n}");
                    n += 1;
                }
                let id = p.fresh_id();
                p.folders.push(Folder { id, name: name.clone(), parent });
                self.current_folder = Some(id);
                self.rename = Some((id, name));
                self.dirty = true;
            }
            NavAction::NewDiagram(kind, folder) => {
                self.current_folder = folder;
                self.add_diagram(kind, None);
            }
            NavAction::RenameFolder(id) => {
                if let Some(f) = p.folder(id) {
                    self.rename = Some((id, f.name.clone()));
                }
            }
            NavAction::DeleteFolder(id) => {
                // what was inside moves up to the folder's parent; no diagram is deleted
                let parent = p.folder(id).and_then(|f| f.parent);
                for d in p.diagrams.iter_mut().filter(|d| d.folder == Some(id)) {
                    d.folder = parent;
                }
                for f in p.folders.iter_mut().filter(|f| f.parent == Some(id)) {
                    f.parent = parent;
                }
                p.folders.retain(|f| f.id != id);
                if self.current_folder == Some(id) {
                    self.current_folder = parent;
                }
                self.dirty = true;
            }
            NavAction::Move(NavDrag::Diagram(did), target) => {
                if let Some(d) = p.diagram_mut(did) {
                    if d.folder != target {
                        d.folder = target;
                        self.dirty = true;
                    }
                }
            }
            NavAction::Move(NavDrag::Folder(fid), target) => {
                if target == Some(fid) || target.map(|t| is_inside(p, t, fid)).unwrap_or(false) {
                    return;
                }
                if folder_level(p, target) + folder_height(p, fid) > MAX_FOLDER_DEPTH {
                    self.status = tr(lang, "Folders can be at most 3 levels deep.").to_string();
                    return;
                }
                if let Some(f) = p.folders.iter_mut().find(|f| f.id == fid) {
                    if f.parent != target {
                        f.parent = target;
                        self.dirty = true;
                    }
                }
            }
        }
    }

    /// Diagram Navigator by diagram type: the project, three groups, each diagram type with
    /// its count, and the diagrams of that type.
    pub(super) fn navigator_by_type(&mut self, ui: &mut egui::Ui, project: &Project) {
        let lang = self.settings.language;
        let groups: [(&str, Vec<DiagramKind>); 4] = [
            ("UML Diagrams", DiagramKind::ALL.iter().copied().filter(|k| !matches!(k, DiagramKind::Erd | DiagramKind::Network | DiagramKind::MindMap)).collect()),
            ("Database Modeling", vec![DiagramKind::Erd]),
            ("Network", vec![DiagramKind::Network]),
            ("Mind Map", vec![DiagramKind::MindMap]),
        ];
        let mut rows: Vec<TreeRow> = vec![];
        let root = TreeKey::Root(1);
        let root_open = self.tree_is_open(root, true);
        rows.push(TreeRow::new(root, 0, format!("🗀 {}", project.name), true, root_open, None));
        if root_open {
            for (gi, (group, kinds)) in groups.iter().enumerate() {
                let gkey = TreeKey::Group(gi as u8);
                let gopen = self.tree_is_open(gkey, true);
                let gidx = rows.len();
                rows.push(TreeRow::new(gkey, 1, tr(lang, group).to_string(), true, gopen, Some(0)));
                if !gopen {
                    continue;
                }
                for kind in kinds {
                    let list: Vec<&Diagram> = project.diagrams.iter().filter(|d| d.kind == *kind).collect();
                    let kkey = TreeKey::Kind(*kind as u8);
                    let kopen = !list.is_empty() && self.tree_is_open(kkey, true);
                    let kidx = rows.len();
                    rows.push(TreeRow::new(kkey, 2, format!("{} ({})", tr(lang, kind.label()), list.len()), !list.is_empty(), kopen, Some(gidx)));
                    if kopen {
                        for d in list {
                            let mut r = TreeRow::new(TreeKey::Diagram(1, d.id), 3, d.name.clone(), false, false, Some(kidx));
                            r.active = self.active == Some(d.id);
                            rows.push(r);
                        }
                    }
                }
            }
        }
        let resps = self.tree_draw(ui, &rows);
        self.tree_common(&rows, &resps);
        for (i, (r, on_arrow)) in resps.iter().enumerate() {
            match rows[i].key {
                TreeKey::Kind(k) => {
                    let Some(kind) = DiagramKind::ALL.iter().copied().find(|x| *x as u8 == k) else { continue };
                    r.context_menu(|ui| {
                        if ui.button(format!("{} {}", tr(lang, "New"), tr(lang, kind.label()))).clicked() {
                            self.add_diagram(kind, None);
                            ui.close_menu();
                        }
                    });
                }
                TreeKey::Diagram(_, did) => {
                    if r.clicked() && !on_arrow {
                        self.open_diagram(did);
                    }
                    let dname = project.diagram(did).map(|d| d.name.clone()).unwrap_or_default();
                    r.context_menu(|ui| {
                        if ui.button(tr(lang, "Open")).clicked() {
                            self.open_diagram(did);
                            ui.close_menu();
                        }
                        if ui.button(tr(lang, "Rename")).clicked() {
                            self.rename = Some((did, dname.clone()));
                            ui.close_menu();
                        }
                        if ui.button(tr(lang, "Duplicate Diagram")).clicked() {
                            self.duplicate_diagram(did);
                            ui.close_menu();
                        }
                        if ui.button(tr(lang, "Delete Diagram")).clicked() {
                            self.ask_delete_diagram(did);
                            ui.close_menu();
                        }
                    });
                }
                _ => {}
            }
        }
        self.tree_rows = rows;
    }

    /// Diagram Navigator by folder: the person's own folders (sorted by name) and the
    /// diagrams in each.
    pub(super) fn navigator_by_folder(&mut self, ui: &mut egui::Ui, project: &Project) {
        let lang = self.settings.language;
        let accent = ui.visuals().selection.bg_fill;
        let mut rows: Vec<TreeRow> = vec![];
        let root = TreeKey::Root(0);
        let root_open = self.tree_is_open(root, true);
        rows.push(TreeRow::new(root, 0, format!("🗀 {}", project.name), true, root_open, None));
        if root_open {
            self.folder_rows(project, None, 1, 0, &mut rows);
        }
        let resps = self.tree_draw(ui, &rows);
        self.tree_common(&rows, &resps);
        let mut actions: Vec<NavAction> = vec![];
        for (i, (r, on_arrow)) in resps.iter().enumerate() {
            let target = match rows[i].key {
                TreeKey::Root(_) => None,
                TreeKey::Folder(fid) => Some(fid),
                _ => {
                    // a diagram: dropping on it puts the dragged item into the diagram's folder
                    rows[i].parent.and_then(|p| match rows[p].key {
                        TreeKey::Folder(f) => Some(f),
                        _ => None,
                    })
                }
            };
            if let Some(p) = r.dnd_release_payload::<NavDrag>() {
                actions.push(NavAction::Move(*p, target));
            }
            if matches!(rows[i].key, TreeKey::Root(_) | TreeKey::Folder(_)) && r.dnd_hover_payload::<NavDrag>().is_some() {
                ui.painter().rect_stroke(r.rect, 2.0, Stroke::new(1.5_f32, accent), StrokeKind::Inside);
            }
            match rows[i].key {
                TreeKey::Root(_) => {
                    if r.clicked() {
                        self.current_folder = None;
                    }
                    r.context_menu(|ui| {
                        if ui.button(tr(lang, "New Folder")).clicked() {
                            actions.push(NavAction::NewFolder(None));
                            ui.close_menu();
                        }
                        ui.menu_button(tr(lang, "New Diagram"), |ui| {
                            for k in DiagramKind::ALL {
                                if ui.button(tr(lang, k.label())).clicked() {
                                    actions.push(NavAction::NewDiagram(k, None));
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                }
                TreeKey::Folder(fid) => {
                    r.dnd_set_drag_payload(NavDrag::Folder(fid));
                    if self.current_folder == Some(fid) {
                        ui.painter().rect_stroke(r.rect.shrink2(Vec2::new(0.0, 1.0)), 2.0, Stroke::new(1.0_f32, accent.linear_multiply(0.6)), StrokeKind::Inside);
                    }
                    if r.clicked() || r.secondary_clicked() {
                        self.current_folder = Some(fid);
                    }
                    let level = rows[i].depth;
                    r.context_menu(|ui| {
                        if ui.add_enabled(level < MAX_FOLDER_DEPTH, egui::Button::new(tr(lang, "New Folder"))).clicked() {
                            actions.push(NavAction::NewFolder(Some(fid)));
                            ui.close_menu();
                        }
                        ui.menu_button(tr(lang, "New Diagram"), |ui| {
                            for k in DiagramKind::ALL {
                                if ui.button(tr(lang, k.label())).clicked() {
                                    actions.push(NavAction::NewDiagram(k, Some(fid)));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.separator();
                        if ui.button(tr(lang, "Rename Folder")).clicked() {
                            actions.push(NavAction::RenameFolder(fid));
                            ui.close_menu();
                        }
                        if ui.button(tr(lang, "Delete Folder")).clicked() {
                            actions.push(NavAction::DeleteFolder(fid));
                            ui.close_menu();
                        }
                    });
                }
                TreeKey::Diagram(_, did) => {
                    r.dnd_set_drag_payload(NavDrag::Diagram(did));
                    if r.clicked() && !on_arrow {
                        self.open_diagram(did);
                        self.current_folder = target;
                    }
                    let dname = project.diagram(did).map(|d| d.name.clone()).unwrap_or_default();
                    r.context_menu(|ui| {
                        if ui.button(tr(lang, "Open")).clicked() {
                            self.open_diagram(did);
                            ui.close_menu();
                        }
                        if ui.button(tr(lang, "Rename")).clicked() {
                            self.rename = Some((did, dname.clone()));
                            ui.close_menu();
                        }
                        if ui.button(tr(lang, "Duplicate Diagram")).clicked() {
                            self.duplicate_diagram(did);
                            ui.close_menu();
                        }
                        ui.menu_button(tr(lang, "Move to Folder"), |ui| {
                            if ui.button(tr(lang, "(Project root)")).clicked() {
                                actions.push(NavAction::Move(NavDrag::Diagram(did), None));
                                ui.close_menu();
                            }
                            for (fid, path) in folder_paths(project) {
                                if ui.button(path).clicked() {
                                    actions.push(NavAction::Move(NavDrag::Diagram(did), Some(fid)));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.separator();
                        if ui.button(tr(lang, "Delete Diagram")).clicked() {
                            self.ask_delete_diagram(did);
                            ui.close_menu();
                        }
                    });
                }
                _ => {}
            }
        }
        self.tree_rows = rows;
        for a in actions {
            self.apply_nav(a);
        }
    }

    /// The rows for the folders and diagrams directly inside `parent` (None = the project
    /// root), `depth` levels in, under the row at `parent_row`.
    fn folder_rows(&self, project: &Project, parent: Option<u64>, depth: usize, parent_row: usize, rows: &mut Vec<TreeRow>) {
        let lang = self.settings.language;
        let mut folders: Vec<&Folder> = project.folders.iter().filter(|f| f.parent == parent).collect();
        folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        for f in folders {
            let key = TreeKey::Folder(f.id);
            let has = project.folders.iter().any(|c| c.parent == Some(f.id)) || project.diagrams.iter().any(|d| d.folder == Some(f.id));
            let open = has && self.tree_is_open(key, true);
            let idx = rows.len();
            rows.push(TreeRow::new(key, depth, format!("🗀 {}", f.name), has, open, Some(parent_row)));
            if open {
                self.folder_rows(project, Some(f.id), depth + 1, idx, rows);
            }
        }
        let mut diagrams: Vec<&Diagram> = project.diagrams.iter().filter(|d| d.folder == parent || (parent.is_none() && d.folder.map(|f| project.folder(f).is_none()).unwrap_or(false))).collect();
        diagrams.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        for d in diagrams {
            let mut r = TreeRow::new(TreeKey::Diagram(0, d.id), depth, format!("▫ {}", d.name), false, false, Some(parent_row));
            r.active = self.active == Some(d.id);
            r.hover = Some(tr(lang, d.kind.label()).to_string());
            rows.push(r);
        }
    }

    /// Model Explorer: every diagram and the shapes in it.
    pub(super) fn explorer(&mut self, ui: &mut egui::Ui, project: &Project) {
        let mut rows: Vec<TreeRow> = vec![];
        let root = TreeKey::Root(2);
        let root_open = self.tree_is_open(root, true);
        rows.push(TreeRow::new(root, 0, format!("🗀 {}", project.name), true, root_open, None));
        if root_open {
            for d in &project.diagrams {
                let key = TreeKey::Diagram(2, d.id);
                let has = !d.shapes.is_empty();
                let open = has && self.tree_is_open(key, self.active == Some(d.id));
                let idx = rows.len();
                let mut r = TreeRow::new(key, 1, d.name.clone(), has, open, Some(0));
                r.active = self.active == Some(d.id);
                rows.push(r);
                if open {
                    for s in &d.shapes {
                        let kind_label = palette::label_for(d.kind, Tool::Shape(s.kind));
                        let name = if s.name.is_empty() { "(unnamed)" } else { s.name.as_str() };
                        let mut r = TreeRow::new(TreeKey::Shape(d.id, s.id), 2, format!("{name}  ·  {kind_label}"), false, false, Some(idx));
                        r.active = self.active == Some(d.id) && self.sel_shapes.contains(&s.id);
                        rows.push(r);
                    }
                }
            }
        }
        let resps = self.tree_draw(ui, &rows);
        self.tree_common(&rows, &resps);
        for (i, (r, on_arrow)) in resps.iter().enumerate() {
            if !r.clicked() || *on_arrow {
                continue;
            }
            match rows[i].key {
                TreeKey::Diagram(_, did) => self.open_diagram(did),
                TreeKey::Shape(did, sid) => self.show_shape(did, sid),
                _ => {}
            }
        }
        self.tree_rows = rows;
    }

    /// Open a diagram with one of its shapes selected and in view.
    fn show_shape(&mut self, did: u64, sid: u64) {
        self.open_diagram(did);
        self.sel_shapes = vec![sid];
        self.sel_conns.clear();
        self.center_on(sid);
    }

    /// Whether a tree row is expanded: the person's choice, else `default`.
    fn tree_is_open(&self, key: TreeKey, default: bool) -> bool {
        self.tree_open.get(&key).copied().unwrap_or(default)
    }

    /// Draw tree rows: an arrow for rows with children, the label, the selected row filled
    /// (full colour while the tree has the keyboard), the open diagram lightly filled. Rows
    /// take clicks and drags but never egui's keyboard focus, so the keys stay the tree's.
    /// Returns each row's response and whether the press was on its arrow.
    fn tree_draw(&mut self, ui: &mut egui::Ui, rows: &[TreeRow]) -> Vec<(egui::Response, bool)> {
        let row_h = 21.0;
        let indent = 14.0;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let vis = ui.visuals().clone();
        let mut out = vec![];
        let pointer = ui.ctx().input(|i| i.pointer.interact_pos());
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in rows {
                let selected = self.tree_sel == Some(row.key);
                let text_color = if selected && self.tree_focus {
                    vis.selection.stroke.color
                } else if row.active {
                    vis.strong_text_color()
                } else {
                    vis.text_color()
                };
                let galley = ui.painter().layout_no_wrap(row.label.clone(), font.clone(), text_color);
                let x_arrow = row.depth as f32 * indent + 8.0;
                let x_text = row.depth as f32 * indent + 18.0;
                let w = (x_text + galley.size().x + 8.0).max(ui.available_width());
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, row_h), Sense::CLICK | Sense::DRAG);
                let fill = if selected {
                    if self.tree_focus { vis.selection.bg_fill } else { vis.selection.bg_fill.linear_multiply(0.6) }
                } else if row.active {
                    vis.selection.bg_fill.linear_multiply(0.45)
                } else if resp.hovered() {
                    vis.widgets.hovered.weak_bg_fill
                } else {
                    Color32::TRANSPARENT
                };
                let p = ui.painter();
                if fill != Color32::TRANSPARENT {
                    p.rect_filled(rect.shrink2(Vec2::new(0.0, 1.0)), 2.0, fill);
                }
                let (ax, cy) = (rect.left() + x_arrow, rect.center().y);
                if row.has_children {
                    let pts = if row.open {
                        vec![Pos2::new(ax - 4.0, cy - 2.0), Pos2::new(ax + 4.0, cy - 2.0), Pos2::new(ax, cy + 3.0)]
                    } else {
                        vec![Pos2::new(ax - 2.0, cy - 4.0), Pos2::new(ax + 3.0, cy), Pos2::new(ax - 2.0, cy + 4.0)]
                    };
                    p.add(egui::Shape::convex_polygon(pts, text_color, Stroke::NONE));
                }
                p.galley(Pos2::new(rect.left() + x_text, cy - galley.size().y / 2.0), galley, text_color);
                if selected && self.tree_reveal {
                    ui.scroll_to_rect(rect, None);
                }
                let on_arrow = row.has_children && pointer.map(|q| q.x < ax + 7.0 && rect.contains(q)).unwrap_or(false);
                let resp = match &row.hover {
                    Some(h) => resp.on_hover_text(h),
                    None => resp,
                };
                out.push((resp, on_arrow));
            }
        });
        self.tree_reveal = false;
        out
    }

    /// What every tree does with the mouse: a click (or right-click) selects the row and
    /// gives the tree the keyboard; a double-click, or a click on the arrow, opens or closes
    /// a row with children.
    fn tree_common(&mut self, rows: &[TreeRow], resps: &[(egui::Response, bool)]) {
        for (row, (r, on_arrow)) in rows.iter().zip(resps) {
            let same_row = self.tree_last_click == Some(row.key);
            if r.clicked() {
                self.tree_last_click = Some(row.key);
            }
            if r.clicked() || r.secondary_clicked() {
                self.tree_sel = Some(row.key);
                self.tree_focus = true;
                if let Some(f) = r.ctx.memory(|m| m.focused()) {
                    r.ctx.memory_mut(|m| m.surrender_focus(f));
                }
            }
            if row.has_children && ((r.clicked() && *on_arrow) || (r.double_clicked() && same_row && !*on_arrow)) {
                self.tree_open.insert(row.key, !row.open);
            }
        }
    }

    /// The keys of the tree that has the keyboard: Up/Down move, Home/End jump, Right opens
    /// a folder (or steps into it when open), Left closes it (or steps out to the parent),
    /// Enter opens a diagram (or a folder), F2 renames.
    pub(super) fn tree_keys(&mut self, ctx: &egui::Context) {
        if !self.tree_focus || !self.show_left || self.tree_rows.is_empty() {
            return;
        }
        let n = Modifiers::NONE;
        let (up, down, left, right, enter, f2, home, end) = ctx.input_mut(|i| {
            (
                i.consume_key(n, Key::ArrowUp),
                i.consume_key(n, Key::ArrowDown),
                i.consume_key(n, Key::ArrowLeft),
                i.consume_key(n, Key::ArrowRight),
                i.consume_key(n, Key::Enter),
                i.consume_key(n, Key::F2),
                i.consume_key(n, Key::Home),
                i.consume_key(n, Key::End),
            )
        });
        if !(up || down || left || right || enter || f2 || home || end) {
            return;
        }
        let rows = std::mem::take(&mut self.tree_rows);
        let Some(i) = self.tree_sel.and_then(|k| rows.iter().position(|r| r.key == k)) else {
            // nothing selected in this tree yet: the first row
            if up || down || home || end {
                self.tree_sel = Some(rows[0].key);
                self.tree_reveal = true;
            }
            self.tree_rows = rows;
            return;
        };
        let row = &rows[i];
        let mut sel = i;
        if up && i > 0 {
            sel = i - 1;
        }
        if down && i + 1 < rows.len() {
            sel = i + 1;
        }
        if home {
            sel = 0;
        }
        if end {
            sel = rows.len() - 1;
        }
        if right && row.has_children {
            if !row.open {
                self.tree_open.insert(row.key, true);
            } else if i + 1 < rows.len() {
                sel = i + 1;
            }
        }
        if left {
            if row.open {
                self.tree_open.insert(row.key, false);
            } else if let Some(p) = row.parent {
                sel = p;
            }
        }
        if sel != i {
            self.tree_sel = Some(rows[sel].key);
            self.tree_reveal = true;
        }
        if enter {
            match row.key {
                TreeKey::Diagram(view, did) => {
                    self.open_diagram(did);
                    if view == 0 {
                        self.current_folder = row.parent.and_then(|p| match rows[p].key {
                            TreeKey::Folder(f) => Some(f),
                            _ => None,
                        });
                    }
                }
                TreeKey::Shape(did, sid) => self.show_shape(did, sid),
                _ if row.has_children => {
                    self.tree_open.insert(row.key, !row.open);
                }
                _ => {}
            }
        }
        if f2 {
            let p = self.project.as_ref();
            match row.key {
                TreeKey::Root(_) => {
                    if let Some(p) = p {
                        self.rename = Some((PROJECT_RENAME_ID, p.name.clone()));
                    }
                }
                TreeKey::Folder(fid) => {
                    if let Some(f) = p.and_then(|p| p.folder(fid)) {
                        self.rename = Some((fid, f.name.clone()));
                    }
                }
                TreeKey::Diagram(_, did) => {
                    if let Some(d) = p.and_then(|p| p.diagram(did)) {
                        self.rename = Some((did, d.name.clone()));
                    }
                }
                TreeKey::Shape(did, sid) => {
                    self.show_shape(did, sid);
                    self.start_edit(Some(EditTarget::Shape(sid)));
                }
                _ => {}
            }
        }
        self.tree_rows = rows;
    }
}
