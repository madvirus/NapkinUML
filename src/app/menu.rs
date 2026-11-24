//! The menu bar, its pop-ups and keyboard navigation, keyboard shortcuts and the toolbar.

use super::*;

/// Everything the menus can do.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Act {
    ToggleAi,
    New,
    Open,
    OpenNewWindow,
    Recent(usize),
    OpenExample,
    Save,
    SaveAs,
    ExportPng,
    ExportMany,
    Print,
    CopyImage,
    CopyOffice,
    ProjectProperties,
    CloseProject,
    Exit,
    NewDiagramDlg,
    NewDiagram(DiagramKind),
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    SelectAll,
    SelectSameType,
    SelectInverse,
    Align(u8),
    SameSize(u8),
    RenameDiagram,
    DuplicateDiagram,
    DeleteDiagram,
    CloseActive,
    CloseAll,
    ZoomIn,
    ZoomOut,
    Zoom100,
    Fit,
    ToggleGrid,
    ToggleSnap,
    ToggleNavigator,
    ToggleExplorer,
    ToggleProperty,
    ToggleToolbar,
    TogglePalette,
    ToggleFullscreen,
    NewWindow,
    Switcher,
    AppOptions,
    ProjectOptions,
    About,
}

/// One row of a menu.
pub(super) enum MItem {
    Act { label: String, shortcut: String, enabled: bool, act: Act, checked: Option<bool> },
    Sub { label: String, enabled: bool, items: Vec<MItem> },
    Sep,
}

/// The open menu: which top menu, and the highlighted row at each open level.
#[derive(Clone)]
pub(super) struct MenuState {
    pub(super) top: usize,
    pub(super) path: Vec<usize>,
}

/// A menu title with its access key underlined ("Project", or "프로젝트(P)").
pub(super) fn mnemonic_job(label: &str, key: char, color: Color32, font: egui::FontId) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let normal = egui::TextFormat { font_id: font, color, ..Default::default() };
    let under = egui::TextFormat { underline: Stroke::new(1.0_f32, color), ..normal.clone() };
    if let Some(pos) = label.find(|c: char| c.eq_ignore_ascii_case(&key)) {
        let (head, rest) = label.split_at(pos);
        let n = rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        job.append(head, 0.0, normal.clone());
        job.append(&rest[..n], 0.0, under);
        job.append(&rest[n..], 0.0, normal);
    } else {
        job.append(label, 0.0, normal.clone());
        job.append("(", 0.0, normal.clone());
        job.append(&key.to_string(), 0.0, under);
        job.append(")", 0.0, normal);
    }
    job
}

pub(super) fn sc(mods: Modifiers, key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(mods, key)
}

pub(super) const SC_NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);

pub(super) const SC_OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);

pub(super) const SC_PRINT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::P);

pub(super) const SC_COPY_IMAGE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::C);

pub(super) const SC_SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);

pub(super) const SC_SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);

pub(super) const SC_UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);

pub(super) const SC_REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

pub(super) const SC_SELECT_ALL: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::A);

pub(super) const SC_DUPLICATE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);

pub(super) const SC_NEW_DIAGRAM: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::D);

pub(super) const SC_ZOOM_IN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Equals);

pub(super) const SC_ZOOM_OUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Minus);

pub(super) const SC_ZOOM_100: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0);

pub(super) const SC_PROPERTY: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::P);

pub(super) const SC_FULLSCREEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F11);

pub(super) const SC_SWITCHER: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::Tab);

pub(super) const SC_NEW_WINDOW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::N);

pub(super) const SC_CLOSE_DIAGRAM: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::W);

impl UmlApp {
    pub(super) fn menu_item(&self, ui: &mut egui::Ui, label: &str, shortcut: Option<&KeyboardShortcut>, enabled: bool) -> bool {
        let mut b = egui::Button::new(self.t(label));
        if let Some(s) = shortcut {
            b = b.shortcut_text(ui.ctx().format_shortcut(s));
        }
        let clicked = ui.add_enabled(enabled, b).clicked();
        if clicked {
            ui.close_menu();
        }
        clicked
    }

    /// The menu bar's contents (Project, Diagram, View, Window) with each item's state.
    pub(super) fn menus(&self, ctx: &egui::Context) -> Vec<(String, char, Vec<MItem>)> {
        let lang = self.settings.language;
        let t = |s: &str| tr(lang, s).to_string();
        let fmt = |s: &KeyboardShortcut| ctx.format_shortcut(s).replace("Equals", "=").replace("Minus", "-").replace("Plus", "+");
        let has_project = self.project.is_some();
        let has_diagram = self.active.is_some();
        let has_shapes = !self.sel_shapes.is_empty();
        let has_sel = has_shapes || !self.sel_conns.is_empty();
        let multi = self.sel_shapes.len() >= 2;
        let a = |label: &str, sc: Option<&KeyboardShortcut>, enabled: bool, act: Act| MItem::Act {
            label: t(label),
            shortcut: sc.map(fmt).unwrap_or_default(),
            enabled,
            act,
            checked: None,
        };
        let c = |label: &str, checked: bool, act: Act| MItem::Act { label: t(label), shortcut: String::new(), enabled: true, act, checked: Some(checked) };
        let recent: Vec<MItem> = self
            .settings
            .recent
            .iter()
            .enumerate()
            .map(|(i, r)| MItem::Act { label: r.clone(), shortcut: String::new(), enabled: true, act: Act::Recent(i), checked: None })
            .collect();
        let project = vec![
            a("New", Some(&SC_NEW), true, Act::New),
            a("Open...", Some(&SC_OPEN), true, Act::Open),
            a("Open with New Window...", None, true, Act::OpenNewWindow),
            MItem::Sub { label: t("Recent Projects"), enabled: !recent.is_empty(), items: recent },
            a("Open Example Project", None, true, Act::OpenExample),
            MItem::Sep,
            a("Save", Some(&SC_SAVE), has_project, Act::Save),
            a("Save as...", Some(&SC_SAVE_AS), has_project, Act::SaveAs),
            MItem::Sep,
            MItem::Sub { label: t("Export"), enabled: has_diagram, items: vec![a("Active Diagram as Image (PNG)...", None, true, Act::ExportPng), a("Multiple Diagrams as Images (PNG)...", None, true, Act::ExportMany)] },
            a("Print...", Some(&SC_PRINT), has_diagram, Act::Print),
            a("Project Properties...", None, has_project, Act::ProjectProperties),
            MItem::Sep,
            a("Close Project", None, has_project, Act::CloseProject),
            a("Exit", None, true, Act::Exit),
        ];
        let kinds: Vec<MItem> = DiagramKind::ALL.iter().map(|k| a(k.label(), None, true, Act::NewDiagram(*k))).collect();
        let aligns = ["By Left Edges", "By Right Edges", "By Top Edges", "By Bottom Edges", "By Horizontal Centers", "By Vertical Centers"];
        let diagram = vec![
            a("New Diagram...", Some(&SC_NEW_DIAGRAM), true, Act::NewDiagramDlg),
            MItem::Sub { label: t("New"), enabled: true, items: kinds },
            MItem::Sep,
            a("Undo", Some(&SC_UNDO), !self.undo.is_empty(), Act::Undo),
            a("Redo", Some(&SC_REDO), !self.redo.is_empty(), Act::Redo),
            MItem::Sep,
            a("Cut", Some(&sc(Modifiers::COMMAND, Key::X)), has_shapes, Act::Cut),
            a("Copy", Some(&sc(Modifiers::COMMAND, Key::C)), has_shapes, Act::Copy),
            a(if has_sel { "Copy Selection as Image" } else { "Copy Diagram as Image" }, Some(&SC_COPY_IMAGE), has_diagram, Act::CopyImage),
            a(if has_sel { "Copy Selection as EMF" } else { "Copy Diagram as EMF" }, None, has_diagram && cfg!(windows), Act::CopyOffice),
            a("Paste", Some(&sc(Modifiers::COMMAND, Key::V)), self.clipboard.is_some() && has_diagram, Act::Paste),
            a("Duplicate", Some(&SC_DUPLICATE), has_shapes, Act::Duplicate),
            a("Delete", Some(&sc(Modifiers::NONE, Key::Delete)), has_sel, Act::Delete),
            MItem::Sep,
            a("Select All", Some(&SC_SELECT_ALL), has_diagram, Act::SelectAll),
            a("Select All of Same Type", None, has_sel, Act::SelectSameType),
            a("Select Inverse", None, has_diagram, Act::SelectInverse),
            MItem::Sep,
            MItem::Sub { label: t("Align Shapes"), enabled: multi, items: aligns.iter().enumerate().map(|(i, l)| a(l, None, true, Act::Align(i as u8))).collect() },
            MItem::Sub {
                label: t("Same Size"),
                enabled: multi,
                items: ["Same Width", "Same Height", "Same Width and Height"].iter().enumerate().map(|(i, l)| a(l, None, true, Act::SameSize(i as u8))).collect(),
            },
            MItem::Sep,
            a("Rename Diagram...", None, has_diagram, Act::RenameDiagram),
            a("Duplicate Diagram", None, has_diagram, Act::DuplicateDiagram),
            a("Delete Diagram", None, has_diagram, Act::DeleteDiagram),
            MItem::Sep,
            a("Close Active Diagram", Some(&SC_CLOSE_DIAGRAM), has_diagram, Act::CloseActive),
            a("Close All Diagrams", None, !self.tabs.is_empty(), Act::CloseAll),
        ];
        let view = vec![
            a("Zoom In", Some(&SC_ZOOM_IN), has_diagram, Act::ZoomIn),
            a("Zoom Out", Some(&SC_ZOOM_OUT), has_diagram, Act::ZoomOut),
            a("Zoom 100% (Actual Size)", Some(&SC_ZOOM_100), has_diagram, Act::Zoom100),
            a("Fit to Window", None, has_diagram, Act::Fit),
            MItem::Sep,
            MItem::Sub { label: t("Grid"), enabled: true, items: vec![c("Show Grid", self.view().show_grid, Act::ToggleGrid), c("Snap to Grid", self.settings.snap, Act::ToggleSnap)] },
            MItem::Sub {
                label: t("Panes"),
                enabled: true,
                items: vec![
                    c("Diagram Navigator", self.show_left && self.left_tab == 0, Act::ToggleNavigator),
                    c("Model Explorer", self.show_left && self.left_tab == 1, Act::ToggleExplorer),
                    MItem::Act { label: t("Property"), shortcut: fmt(&SC_PROPERTY), enabled: true, act: Act::ToggleProperty, checked: Some(self.show_property) },
                ],
            },
            c("Toolbar", self.show_toolbar, Act::ToggleToolbar),
            c("Diagram Toolbar", self.show_palette, Act::TogglePalette),
            c("AI Assistant", self.ai.show, Act::ToggleAi),
            MItem::Sep,
            MItem::Act { label: t("Full Screen"), shortcut: fmt(&SC_FULLSCREEN), enabled: true, act: Act::ToggleFullscreen, checked: Some(self.fullscreen) },
        ];
        let window = vec![
            a("New Window", Some(&SC_NEW_WINDOW), true, Act::NewWindow),
            a("Window Switcher", Some(&SC_SWITCHER), has_project, Act::Switcher),
            MItem::Sep,
            a("Application Options...", None, true, Act::AppOptions),
            a("Project Options...", None, has_project, Act::ProjectOptions),
        ];
        let help = vec![a("About NapkinUML", None, true, Act::About)];
        vec![(t("Project"), 'P', project), (t("Diagram"), 'D', diagram), (t("View"), 'V', view), (t("Window"), 'W', window), (t("Help"), 'H', help)]
    }

    pub(super) fn run_act(&mut self, ctx: &egui::Context, act: Act) {
        match act {
            Act::New => self.request(Pending::New),
            Act::Open => self.request(Pending::Open),
            Act::OpenNewWindow => self.open_in_new_window(),
            Act::Recent(i) => {
                if let Some(r) = self.settings.recent.get(i).cloned() {
                    self.request(Pending::OpenPath(PathBuf::from(r)));
                }
            }
            Act::Save => {
                self.save();
            }
            Act::SaveAs => {
                self.save_as();
            }
            Act::ExportPng => self.export_image(),
            Act::ExportMany => self.open_export_dialog(),
            Act::Print => self.open_print_dialog(),
            Act::ProjectProperties => self.open_project_options(0),
            Act::CloseProject => self.request(Pending::Close),
            Act::Exit => self.request(Pending::Exit),
            Act::OpenExample => self.request(Pending::Example),
            Act::NewDiagramDlg => self.new_diagram = Some((DiagramKind::Class, String::new())),
            Act::NewDiagram(k) => self.add_diagram(k, None),
            Act::Undo => self.undo(),
            Act::Redo => self.redo(),
            Act::Cut => self.cut(),
            Act::Copy => {
                self.copy_selection();
            }
            Act::CopyImage => self.copy_as_image(),
            Act::CopyOffice => self.copy_for_office(None),
            Act::Paste => self.paste(),
            Act::Duplicate => self.duplicate(),
            Act::Delete => self.delete_selection(),
            Act::SelectAll => self.select_all(),
            Act::SelectSameType => self.select_same_type(),
            Act::SelectInverse => self.select_inverse(),
            Act::Align(i) => self.align(i),
            Act::SameSize(i) => self.same_size(i),
            Act::RenameDiagram => {
                if let Some(d) = self.diag() {
                    self.rename = Some((d.id, d.name.clone()));
                }
            }
            Act::DuplicateDiagram => {
                if let Some(id) = self.active {
                    self.duplicate_diagram(id);
                }
            }
            Act::DeleteDiagram => {
                if let Some(id) = self.active {
                    self.ask_delete_diagram(id);
                }
            }
            Act::CloseActive => {
                if let Some(id) = self.active {
                    self.close_diagram(id);
                }
            }
            Act::CloseAll => {
                self.tabs.clear();
                self.active = None;
                self.clear_selection();
            }
            Act::ZoomIn => self.zoom_in_step(None),
            Act::ZoomOut => self.zoom_out_step(None),
            Act::Zoom100 => self.zoom_to(1.0),
            Act::Fit => self.fit_to_window(),
            Act::ToggleGrid => {
                if let Some(v) = self.view_mut() {
                    v.show_grid = !v.show_grid;
                }
            }
            Act::ToggleSnap => {
                self.settings.snap = !self.settings.snap;
                self.settings.save();
            }
            Act::ToggleNavigator => {
                let on = self.show_left && self.left_tab == 0;
                self.show_left = !on;
                self.left_tab = 0;
            }
            Act::ToggleExplorer => {
                let on = self.show_left && self.left_tab == 1;
                self.show_left = !on;
                self.left_tab = 1;
            }
            Act::ToggleProperty => self.show_property = !self.show_property,
            Act::ToggleAi => self.ai.show = !self.ai.show,
            Act::ToggleToolbar => self.show_toolbar = !self.show_toolbar,
            Act::TogglePalette => self.show_palette = !self.show_palette,
            Act::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            Act::NewWindow => self.new_window(),
            Act::Switcher => self.open_switcher(),
            Act::AppOptions => self.app_opts = Some(AppOptionsDlg { s: self.settings.clone(), page: 0 }),
            Act::ProjectOptions => self.open_project_options(1),
            Act::About => self.about_open = true,
        }
    }

    /// The menu bar: Project (Alt+P), Diagram (Alt+D), View (Alt+V), Window (Alt+W).
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let menus = self.menus(ui.ctx());
        let mut clicked: Option<usize> = None;
        let mut hovered: Option<usize> = None;
        let font = egui::TextStyle::Button.resolve(ui.style());
        let color = ui.visuals().text_color();
        ui.horizontal(|ui| {
            for (i, (label, key, _)) in menus.iter().enumerate() {
                let open = self.menu.as_ref().map(|m| m.top == i).unwrap_or(false);
                let job = mnemonic_job(label, *key, color, font.clone());
                let r = ui.add(egui::SelectableLabel::new(open, job)).on_hover_text(format!("Alt+{key}"));
                if i < self.menu_top_rects.len() {
                    self.menu_top_rects[i] = r.rect;
                }
                if r.clicked() {
                    clicked = Some(i);
                }
                if r.hovered() {
                    hovered = Some(i);
                }
            }
        });
        if let Some(i) = clicked {
            let same = self.menu.as_ref().map(|m| m.top == i).unwrap_or(false);
            self.menu = if same { None } else { Some(MenuState { top: i, path: vec![usize::MAX] }) };
        } else if let (Some(i), true, Some(m)) = (hovered, ui.ctx().input(|inp| inp.pointer.delta() != Vec2::ZERO), self.menu.as_mut()) {
            if m.top != i {
                m.top = i;
                m.path = vec![usize::MAX];
            }
        }
    }

    /// The open menu (and its open submenus), drawn above everything.
    pub(super) fn menu_popups(&mut self, ctx: &egui::Context) {
        let Some(state) = self.menu.clone() else { return };
        let menus = self.menus(ctx);
        let (Some((_, _, items)), Some(top_rect)) = (menus.get(state.top), self.menu_top_rects.get(state.top).copied()) else {
            self.menu = None;
            return;
        };
        let mut pos = top_rect.left_bottom();
        let mut level_items: &Vec<MItem> = items;
        let mut rects: Vec<Rect> = self.menu_top_rects.to_vec();
        let mut action: Option<Act> = None;
        let mut new_path: Option<Vec<usize>> = None;
        let accent = ctx.style().visuals.selection.bg_fill;
        for level in 0..state.path.len() {
            let hl = state.path[level];
            let mut row_rects: Vec<Rect> = vec![];
            let prefix: Vec<usize> = state.path[..level].to_vec();
            // the popup is as wide as its longest label plus its shortcut
            let bf = egui::TextStyle::Button.resolve(&ctx.style());
            let measure = |s: &str| ctx.fonts(|f| f.layout_no_wrap(s.to_string(), bf.clone(), Color32::WHITE).size().x);
            let width = level_items
                .iter()
                .map(|it| match it {
                    MItem::Act { label, shortcut, .. } => measure(label) + measure(shortcut) + 70.0,
                    MItem::Sub { label, .. } => measure(label) + 70.0,
                    MItem::Sep => 0.0,
                })
                .fold(200.0_f32, f32::max);
            let resp = egui::Area::new(egui::Id::new(("menu_popup", level))).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, it) in level_items.iter().enumerate() {
                        let (label, shortcut, enabled, checked, sub) = match it {
                            MItem::Sep => {
                                ui.add_space(3.0);
                                ui.separator();
                                ui.add_space(3.0);
                                row_rects.push(Rect::NOTHING);
                                continue;
                            }
                            MItem::Act { label, shortcut, enabled, checked, .. } => (label.as_str(), shortcut.as_str(), *enabled, *checked, false),
                            MItem::Sub { label, enabled, .. } => (label.as_str(), "", *enabled, None, true),
                        };
                        let (rect, r) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click());
                        row_rects.push(rect);
                        let lit = i == hl && enabled;
                        if lit {
                            ui.painter().rect_filled(rect, 3.0, accent);
                        }
                        let col = if lit {
                            Color32::WHITE
                        } else if enabled {
                            ui.visuals().text_color()
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        let f = egui::TextStyle::Button.resolve(ui.style());
                        if checked == Some(true) {
                            ui.painter().text(Pos2::new(rect.left() + 6.0, rect.center().y), Align2::LEFT_CENTER, "✔", f.clone(), col);
                        }
                        ui.painter().text(Pos2::new(rect.left() + 24.0, rect.center().y), Align2::LEFT_CENTER, label, f.clone(), col);
                        if sub {
                            // a small triangle points to the submenu
                            let c = Pos2::new(rect.right() - 12.0, rect.center().y);
                            ui.painter().add(egui::epaint::Shape::convex_polygon(vec![c + Vec2::new(-3.0, -4.5), c + Vec2::new(3.0, 0.0), c + Vec2::new(-3.0, 4.5)], col, Stroke::NONE));
                        } else {
                            let sc_col = if lit { Color32::WHITE } else { ui.visuals().weak_text_color() };
                            ui.painter().text(Pos2::new(rect.right() - 8.0, rect.center().y), Align2::RIGHT_CENTER, shortcut, f, sc_col);
                        }
                        if r.hovered() && enabled && ctx.input(|inp| inp.pointer.delta() != Vec2::ZERO) {
                            let mut p = prefix.clone();
                            p.push(i);
                            if sub {
                                p.push(usize::MAX);
                            }
                            new_path = Some(p);
                        }
                        if r.clicked() && enabled {
                            if let MItem::Act { act, .. } = it {
                                action = Some(*act);
                            } else {
                                let mut p = prefix.clone();
                                p.push(i);
                                p.push(usize::MAX);
                                new_path = Some(p);
                            }
                        }
                    }
                });
            });
            rects.push(resp.response.rect);
            if level + 1 < state.path.len() {
                let i = state.path[level];
                match level_items.get(i) {
                    Some(MItem::Sub { items, .. }) if i < row_rects.len() => {
                        pos = row_rects[i].right_top() + Vec2::new(6.0, -4.0);
                        level_items = items;
                    }
                    _ => break,
                }
            }
        }
        if let Some(act) = action {
            self.menu = None;
            self.run_act(ctx, act);
            return;
        }
        if let Some(p) = new_path {
            if let Some(m) = self.menu.as_mut() {
                m.path = p;
            }
        }
        // a click anywhere else closes the menu
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                if !rects.iter().any(|r| r.contains(p)) {
                    self.menu = None;
                }
            }
        }
    }

    /// Keyboard for the menus: Alt+P/D/V/W/H (and F10) open them; arrows move,
    /// Enter/Space choose, Esc closes. Returns true while a menu is open.
    pub(super) fn menu_keys(&mut self, ctx: &egui::Context) -> bool {
        for (k, i) in [(Key::P, 0usize), (Key::D, 1), (Key::V, 2), (Key::W, 3), (Key::H, 4)] {
            if ctx.input_mut(|inp| inp.consume_key(Modifiers::ALT, k)) {
                self.open_menu_by_key(ctx, i);
            }
        }
        if self.menu.is_none() && ctx.input_mut(|inp| inp.consume_key(Modifiers::NONE, Key::F10)) {
            self.open_menu_by_key(ctx, 0);
        }
        let Some(mut st) = self.menu.clone() else { return false };
        let menus = self.menus(ctx);
        let n = menus.len();
        let items_at = |top: usize, path: &[usize]| -> Option<&Vec<MItem>> {
            let mut items = &menus.get(top)?.2;
            for &i in &path[..path.len().saturating_sub(1)] {
                match items.get(i) {
                    Some(MItem::Sub { items: sub, .. }) => items = sub,
                    _ => return None,
                }
            }
            Some(items)
        };
        let selectable = |it: &MItem| match it {
            MItem::Sep => false,
            MItem::Act { enabled, .. } | MItem::Sub { enabled, .. } => *enabled,
        };
        let step = |items: &Vec<MItem>, cur: usize, down: bool| -> usize {
            let len = items.len();
            if len == 0 {
                return usize::MAX;
            }
            let mut i = if cur == usize::MAX { if down { len - 1 } else { 0 } } else { cur };
            for _ in 0..len {
                i = if down { (i + 1) % len } else { (i + len - 1) % len };
                if selectable(&items[i]) {
                    return i;
                }
            }
            cur
        };
        let pressed = |k: Key| ctx.input_mut(|inp| inp.consume_key(Modifiers::NONE, k));
        let mut run: Option<Act> = None;
        let mut close = false;
        if let Some(items) = items_at(st.top, &st.path) {
            let cur = *st.path.last().unwrap_or(&usize::MAX);
            if pressed(Key::ArrowDown) {
                *st.path.last_mut().unwrap() = step(items, cur, true);
            } else if pressed(Key::ArrowUp) {
                *st.path.last_mut().unwrap() = step(items, cur, false);
            } else if pressed(Key::ArrowRight) {
                match items.get(cur) {
                    Some(MItem::Sub { items: sub, enabled: true, .. }) => st.path.push(step(sub, usize::MAX, true)),
                    _ => {
                        st.top = (st.top + 1) % n;
                        st.path = vec![step(&menus[st.top].2, usize::MAX, true)];
                    }
                }
            } else if pressed(Key::ArrowLeft) {
                if st.path.len() > 1 {
                    st.path.pop();
                } else {
                    st.top = (st.top + n - 1) % n;
                    st.path = vec![step(&menus[st.top].2, usize::MAX, true)];
                }
            } else if pressed(Key::Enter) || pressed(Key::Space) {
                match items.get(cur) {
                    Some(MItem::Act { act, enabled: true, .. }) => run = Some(*act),
                    Some(MItem::Sub { items: sub, enabled: true, .. }) => st.path.push(step(sub, usize::MAX, true)),
                    _ => {}
                }
            } else if pressed(Key::Escape) {
                if st.path.len() > 1 {
                    st.path.pop();
                } else {
                    close = true;
                }
            }
        }
        if let Some(act) = run {
            self.menu = None;
            self.run_act(ctx, act);
        } else if close {
            self.menu = None;
        } else {
            self.menu = Some(st);
        }
        true
    }

    pub(super) fn open_menu_by_key(&mut self, ctx: &egui::Context, top: usize) {
        let menus = self.menus(ctx);
        let first = menus
            .get(top)
            .and_then(|m| m.2.iter().position(|it| matches!(it, MItem::Act { enabled: true, .. } | MItem::Sub { enabled: true, .. })))
            .unwrap_or(usize::MAX);
        self.menu = Some(MenuState { top, path: vec![first] });
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let lang = self.settings.language;
            let b = |ui: &mut egui::Ui, icon: &str, tip: &str, enabled: bool| -> bool {
                ui.add_enabled(enabled, egui::Button::new(egui::RichText::new(icon).size(15.0)).min_size(Vec2::new(28.0, 26.0)).frame(false))
                    .on_hover_text(tr(lang, tip))
                    .clicked()
            };
            if pane_button(ui, self.show_left, false).on_hover_text(tr(lang, "Show/Hide Diagram Navigator and Model Explorer")).clicked() {
                self.show_left = !self.show_left;
            }
            ui.separator();
            if b(ui, "🗋", "New Project", true) {
                self.request(Pending::New);
            }
            if b(ui, "🗁", "Open Project", true) {
                self.request(Pending::Open);
            }
            if b(ui, "💾", "Save Project", self.project.is_some()) {
                self.save();
            }
            ui.separator();
            if b(ui, "⟲", "Undo", !self.undo.is_empty()) {
                self.undo();
            }
            if b(ui, "⟳", "Redo", !self.redo.is_empty()) {
                self.redo();
            }
            ui.separator();
            let has_sel = !self.sel_shapes.is_empty();
            if b(ui, "✂", "Cut", has_sel) {
                self.cut();
            }
            if b(ui, "🗐", "Copy", has_sel) {
                self.copy_selection();
            }
            if b(ui, "📋", "Paste", self.clipboard.is_some() && self.active.is_some()) {
                self.paste();
            }
            if b(ui, "🗑", "Delete", has_sel || !self.sel_conns.is_empty()) {
                self.delete_selection();
            }
            ui.separator();
            let has_d = self.active.is_some();
            if b(ui, "➖", "Zoom Out", has_d) {
                self.zoom_out_step(None);
            }
            let z = (self.view().zoom * 100.0).round();
            egui::ComboBox::from_id_salt("zoom_combo").width(70.0).selected_text(format!("{z}%")).show_ui(ui, |ui| {
                for p in ZOOM_PRESETS {
                    if ui.selectable_label(z as i32 == p, format!("{p}%")).clicked() {
                        self.zoom_to(p as f32 / 100.0);
                    }
                }
            });
            if b(ui, "➕", "Zoom In", has_d) {
                self.zoom_in_step(None);
            }
            if ui.add_enabled(has_d, egui::Button::new(tr(lang, "Fit to Window")).frame(false)).clicked() {
                self.fit_to_window();
            }
            ui.separator();
            ui.menu_button(format!("➕ {}", tr(lang, "New Diagram...").trim_end_matches("...")), |ui| {
                for k in DiagramKind::ALL {
                    if ui.button(tr(lang, k.label())).clicked() {
                        ui.close_menu();
                        self.add_diagram(k, None);
                    }
                }
            });
            ui.separator();
            if ui.add(egui::Button::new(format!("✨ {}", tr(lang, "AI Assistant"))).selected(self.ai.show).frame(false)).on_hover_text(tr(lang, "Draw or change diagrams by talking to an AI agent installed on this computer")).clicked() {
                self.ai.show = !self.ai.show;
            }
            // the Property pane's switch sits at the right end, above the pane
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tip = format!("{} ({})", tr(lang, "Show/Hide Property"), ui.ctx().format_shortcut(&SC_PROPERTY));
                if pane_button(ui, self.show_property, true).on_hover_text(tip).clicked() {
                    self.show_property = !self.show_property;
                }
            });
        });
    }

    pub(super) fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.menu_keys(ctx) {
            return;
        }
        let typing = ctx.wants_keyboard_input();
        let dialog_open = self.switcher.is_some() || self.jobs.any_open() || self.confirm.is_some() || self.confirm_delete.is_some() || self.new_diagram.is_some() || self.app_opts.is_some() || self.proj_opts.is_some() || self.rename.is_some();
        if ctx.input_mut(|i| i.consume_shortcut(&SC_SAVE_AS)) {
            self.save_as();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_PRINT)) {
            self.open_print_dialog();
        }
        let mut image_copied = false;
        if ctx.input_mut(|i| i.consume_shortcut(&SC_COPY_IMAGE)) {
            self.copy_as_image();
            image_copied = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_SAVE)) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_NEW_WINDOW)) {
            self.new_window();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_NEW)) {
            self.request(Pending::New);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_OPEN)) {
            self.request(Pending::Open);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_NEW_DIAGRAM)) {
            self.new_diagram = Some((DiagramKind::Class, String::new()));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_PROPERTY)) {
            self.show_property = !self.show_property;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_FULLSCREEN)) {
            self.fullscreen = !self.fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        }
        if self.switcher.is_none() && ctx.input_mut(|i| i.consume_shortcut(&SC_SWITCHER)) {
            self.open_switcher();
        }
        // Tab on a mind map topic (also while its name is being typed): a sub-topic
        if let Some(id) = self.mind_pending.take() {
            if self.editing.is_some() {
                self.commit_edit();
            }
            self.mind_add(id, true);
            return;
        }
        if typing || dialog_open {
            return;
        }
        // while the left-pane tree has the keyboard, arrows, Enter and F2 are its own
        self.tree_keys(ctx);
        // Enter on a selected mind map topic: a sibling topic (a sub-topic of the central one)
        if !self.tree_focus && ctx.input(|i| i.modifiers.is_none() && i.key_pressed(Key::Enter)) {
            if let Some(id) = self.mind_target(ctx) {
                self.mind_add(id, false);
                return;
            }
        }
        // Ctrl+/ on a mind map topic folds or unfolds its sub-topics
        if !self.tree_focus && ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Slash)) {
            if let Some(id) = self.mind_target(ctx) {
                ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Slash));
                self.mind_toggle(id);
                return;
            }
        }
        // arrow keys on a selected mind map topic select the topic that way (Shift+arrow still nudges)
        if !self.tree_focus {
            if let Some(id) = self.mind_target(ctx) {
                let dir = ctx.input(|i| {
                    if !i.modifiers.is_none() {
                        None
                    } else if i.key_pressed(Key::ArrowLeft) {
                        Some((-1, 0))
                    } else if i.key_pressed(Key::ArrowRight) {
                        Some((1, 0))
                    } else if i.key_pressed(Key::ArrowUp) {
                        Some((0, -1))
                    } else if i.key_pressed(Key::ArrowDown) {
                        Some((0, 1))
                    } else {
                        None
                    }
                });
                if let Some(dir) = dir {
                    self.mind_step(id, dir);
                    return;
                }
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_CLOSE_DIAGRAM)) {
            if let Some(id) = self.active {
                self.close_diagram(id);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_UNDO)) {
            self.undo();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_REDO) || i.consume_shortcut(&sc(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z))) {
            self.redo();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_SELECT_ALL)) {
            self.select_all();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_DUPLICATE)) {
            self.duplicate();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_ZOOM_IN) || i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Plus))) {
            self.zoom_in_step(None);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_ZOOM_OUT)) {
            self.zoom_out_step(None);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_ZOOM_100)) {
            self.zoom_to(1.0);
        }
        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            self.delete_selection();
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.tool = Tool::Select;
            self.drag = Drag::None;
            self.clear_selection();
        }
        if ctx.input(|i| i.key_pressed(Key::F2)) {
            if let Some(&id) = self.sel_shapes.first() {
                self.start_edit(Some(EditTarget::Shape(id)));
            } else if let Some(&id) = self.sel_conns.first() {
                self.start_edit(Some(EditTarget::Conn(id)));
            }
        }
        // clipboard keys arrive as events
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Copy => {
                    // Ctrl+Shift+C reaches us as a Copy with Shift held
                    if ctx.input(|i| i.modifiers.shift) {
                        if !image_copied {
                            self.copy_as_image();
                            image_copied = true;
                        }
                    } else {
                        self.copy_selection();
                    }
                }
                egui::Event::Cut => self.cut(),
                egui::Event::Paste(_) => self.paste(),
                _ => {}
            }
        }
        // arrow keys nudge the selection
        let step = if ctx.input(|i| i.modifiers.shift) { 10.0 } else { 1.0 };
        let mut dv = Vec2::ZERO;
        ctx.input(|i| {
            if i.key_pressed(Key::ArrowLeft) {
                dv.x -= step;
            }
            if i.key_pressed(Key::ArrowRight) {
                dv.x += step;
            }
            if i.key_pressed(Key::ArrowUp) {
                dv.y -= step;
            }
            if i.key_pressed(Key::ArrowDown) {
                dv.y += step;
            }
        });
        if dv != Vec2::ZERO && !self.sel_shapes.is_empty() {
            self.edit_checkpoint((0, 200));
            let ids = self.sel_shapes.clone();
            if let Some(d) = self.diag_mut() {
                let locked: Vec<u64> = d.shapes.iter().filter(|s| y_locked(d, s)).map(|s| s.id).collect();
                for s in d.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
                    s.x += dv.x;
                    if !locked.contains(&s.id) {
                        s.y += dv.y;
                    }
                }
            }
        }
    }
}
