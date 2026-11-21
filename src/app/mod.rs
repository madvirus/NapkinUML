//! The main window: menus, toolbar, panes, canvas and dialogs.

use crate::i18n::{tr, Language};
use crate::model::{ConnKind, Connector, Diagram, DiagramKind, ErdNotation, Folder, Project, ProjectOptions, Shape, ShapeKind};
use crate::model::{attached_ports, is_interface_end, joined_interfaces, opposite_interface, port_owner, press_lane, set_divider, shift_lane_contents, snap_port};
use crate::seq::{bar_at, lifeline_head, seq_menu_target, stretch_activation, y_locked};
use crate::conn_create::{counts_as_empty, pick_choices, pick_default};
use crate::palette::{self, Tool};
use crate::render::{self, Cam, Look};
use crate::settings::Settings;
use egui::{Align2, Color32, Key, KeyboardShortcut, Modifiers, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::HashMap;
use std::path::PathBuf;

mod project;
mod ai_panel;
mod updater;
mod edit;
mod view;
mod export;
mod menu;
mod navigator;
mod properties;
mod canvas;
mod canvas_input;
mod conn_pick;
mod inline_edit;
mod dialogs;
mod screenshot;
use project::*;
use updater::*;
use export::*;
use menu::*;
use navigator::*;
use properties::*;
use canvas::*;
use conn_pick::*;
use inline_edit::*;
use dialogs::*;
use screenshot::Player;

const FILE_EXT: &str = "umlp";
/// Zoom limits from the original: Zoom In stops at 8.0 (v.ddp.j.bD), Zoom Out refuses 0.02 or less (v.ddp.j.bE).
const ZOOM_MAX: f32 = 8.0;
const ZOOM_MIN: f32 = 0.02;
/// The original's fixed zoom levels (ZoomSelection25/50/75/100/150/200Action).
const ZOOM_PRESETS: [i32; 6] = [25, 50, 75, 100, 150, 200];

#[derive(Clone, Copy)]
struct ViewState {
    pan: Vec2,
    zoom: f32,
    show_grid: bool,
}

enum Drag {
    None,
    Move { start: (f32, f32), items: Vec<(u64, f32, f32)>, lines: Vec<(u64, Vec<(f32, f32)>)> },
    MovePoint { id: u64, idx: usize },
    MoveLabel { target: LabelTarget, start: (f32, f32), orig: (f32, f32) },
    MoveEnd { id: u64, to_end: bool },
    /// Dragging a whole line: where its ends and bend points were when the drag began.
    MoveLine { id: u64, start: (f32, f32), from0: (f32, f32), to0: (f32, f32), points0: Vec<(f32, f32)> },
    Resize { id: u64, corner: usize, orig: (f32, f32, f32, f32), start: (f32, f32) },
    Rubber { start: Pos2, cur: Pos2 },
    Connect { from: u64, start: (f32, f32), cur: Pos2 },
    MoveMessage { id: u64, orig: f32, start: f32 },
    /// Dragging the top or bottom handle of a selected activation: the bar as it was, and the
    /// lifeline's splits and stretches before the drag (each frame starts again from them).
    BarEdge { lifeline: u64, depth: usize, top: bool, y0: f32, y1: f32, splits0: Vec<f32>, spans0: Vec<[f32; 3]> },
    /// Dragging a section line of an alternative fragment (its height when the drag began).
    Divider { id: u64, idx: usize, orig: f32 },
    /// Dragging the line after partition `idx` of a swimlane (the sizes when the drag began).
    PartEdge { id: u64, idx: usize, vertical: bool, orig: Vec<f32> },
}

#[derive(Clone)]
enum Pending {
    New,
    Example,
    Open,
    OpenPath(PathBuf),
    Close,
    Exit,
    /// Start the updated program (opening this project) and close this one.
    Restart(PathBuf),
}

enum Shot {
    Check(PathBuf),
    /// A picture taken by a NAPKINUML_PLAY step; the run goes on.
    Step(PathBuf),
}

#[derive(Clone, Copy, PartialEq)]
enum EditTarget {
    Shape(u64),
    Conn(u64),
}

/// A handle of the selected line: a bend point, or the middle dot of a segment.
enum PointHit {
    Point(usize),
    Mid(usize),
}

pub struct UmlApp {
    settings: Settings,
    project: Option<Project>,
    path: Option<PathBuf>,
    dirty: bool,
    tabs: Vec<u64>,
    active: Option<u64>,
    views: HashMap<u64, ViewState>,
    tool: Tool,
    sticky: bool,
    sel_shapes: Vec<u64>,
    sel_conns: Vec<u64>,
    drag: Drag,
    undo: Vec<(u64, Diagram)>,
    redo: Vec<(u64, Diagram)>,
    clipboard: Option<(Vec<Shape>, Vec<Connector>)>,
    editing: Option<InlineEdit>,
    new_diagram: Option<(DiagramKind, String)>,
    app_opts: Option<AppOptionsDlg>,
    /// Help > About NapkinUML is open.
    about_open: bool,
    /// The app icon shown in the About dialog, loaded the first time it opens.
    about_icon: Option<egui::TextureHandle>,
    proj_opts: Option<ProjectOptionsDlg>,
    switcher: Option<usize>,
    rename: Option<(u64, String)>,
    confirm: Option<Pending>,
    confirm_delete: Option<u64>,
    left_tab: usize,
    show_left: bool,
    show_property: bool,
    /// AI Assistant panel and its conversation (true = the person's message).
    show_ai: bool,
    ai_agents: Vec<crate::ai::Found>,
    ai_chat: Vec<(bool, String)>,
    ai_input: String,
    ai_job: Option<crate::ai::Job>,
    update: UpdateState,
    show_toolbar: bool,
    show_palette: bool,
    fullscreen: bool,
    screenshot: Option<(PathBuf, u32)>,
    shot: Option<Shot>,
    /// NAPKINUML_PLAY: scripted clicks and keys for --screenshot runs.
    player: Option<Player>,
    /// The fonts given to egui, also used to draw diagrams off screen.
    font_defs: egui::FontDefinitions,
    /// Project > Export > Multiple Diagrams: the dialog, then the export in progress.
    export_dlg: Option<ExportDlg>,
    batch: Option<BatchExport>,
    /// Project > Print: the dialog, then the pictures being taken for the pages.
    print_dlg: Option<PrintDlg>,
    printing: Option<PrintJob>,
    /// The progress window's Cancel was pressed.
    job_cancel: bool,
    /// Right-clicked on a sequence lifeline: (lifeline, height, on an activation bar, the
    /// split point near the click).
    seq_menu: Option<(u64, f32, bool, Option<f32>)>,
    /// The selected activation bar: (lifeline, depth, a height inside it).
    sel_bar: Option<(u64, usize, f32)>,
    /// Right-clicked inside an alternative fragment: (fragment, height below its top).
    alt_menu: Option<(u64, f32)>,
    /// Connect and Create: a line released on empty space waits here while the person picks
    /// what to create at that point.
    conn_pick: Option<ConnPick>,
    /// Right-clicked inside a swimlane: (swimlane, distance along it).
    lane_menu: Option<(u64, f32)>,
    /// Screen pixels per point, as of this frame.
    ppp: f32,
    status: String,
    canvas_rect: Rect,
    prop_session: Option<(u64, u8)>,
    fit_request: bool,
    allow_close: bool,
    title: String,
    menu: Option<MenuState>,
    menu_top_rects: [Rect; 5],
    nav_by_type: bool,
    current_folder: Option<u64>,
    label_rects: Vec<(LabelTarget, Rect)>,
    rename_focused: Option<u64>,
    /// Which tree rows the person opened or closed (others use their default).
    tree_open: HashMap<TreeKey, bool>,
    /// The selected row of the left-pane tree.
    tree_sel: Option<TreeKey>,
    /// The left-pane tree has the keyboard (a row was clicked; a press outside the pane ends it).
    tree_focus: bool,
    /// The rows the left-pane tree drew last frame, for its keys.
    tree_rows: Vec<TreeRow>,
    /// Scroll the selected row into view when it's next drawn.
    tree_reveal: bool,
    /// The tree row the last click landed on: a double-click opens or closes a row only when
    /// both of its clicks were on that row.
    tree_last_click: Option<TreeKey>,
    font_loaded: Vec<String>,
    font_pending: Vec<String>,
    fonts_ready: bool,
    fonts_check: bool,
}

/// A toolbar button drawn as a window with its left pane (filled when the pane is shown).
/// A pane on/off button drawn as a window with its side pane (left or right) filled when shown.
fn pane_button(ui: &mut egui::Ui, on: bool, right: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(28.0, 26.0), egui::Sense::click());
    let v = ui.visuals();
    if resp.hovered() {
        ui.painter().rect_filled(rect.shrink(1.0), 3.0, v.widgets.hovered.weak_bg_fill);
    }
    let ink = if resp.hovered() { v.strong_text_color() } else { v.text_color() };
    let r = Rect::from_center_size(rect.center(), Vec2::new(16.0, 13.0));
    let side = if right { Rect::from_min_max(Pos2::new(r.right() - 5.5, r.top()), r.max) } else { Rect::from_min_max(r.min, Pos2::new(r.left() + 5.5, r.bottom())) };
    if on {
        ui.painter().rect_filled(side, 1.0, ink);
    }
    ui.painter().rect_stroke(r, 1.5, Stroke::new(1.3_f32, ink), egui::StrokeKind::Middle);
    let edge = if right { side.left() } else { side.right() };
    ui.painter().line_segment([Pos2::new(edge, r.top()), Pos2::new(edge, r.bottom())], Stroke::new(1.3_f32, ink));
    resp
}

impl UmlApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, empty: bool, open: Option<PathBuf>, start_diagram: Option<usize>) -> Self {
        let mut settings = Settings::load();
        if screenshot.is_some() {
            // checks always see the same, default-looking window
            settings = Settings { recent: vec![], ..Settings::default() };
            crate::settings::READ_ONLY.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let mut app = UmlApp {
            settings,
            project: None,
            path: None,
            dirty: false,
            tabs: vec![],
            active: None,
            views: HashMap::new(),
            tool: Tool::Select,
            sticky: false,
            sel_shapes: vec![],
            sel_conns: vec![],
            drag: Drag::None,
            undo: vec![],
            redo: vec![],
            clipboard: None,
            editing: None,
            new_diagram: None,
            app_opts: None,
            about_open: false,
            about_icon: None,
            proj_opts: None,
            switcher: None,
            rename: None,
            confirm: None,
            confirm_delete: None,
            left_tab: 0,
            show_left: true,
            show_property: true,
            show_ai: false,
            ai_agents: crate::ai::detect(),
            ai_chat: vec![],
            ai_input: String::new(),
            ai_job: None,
            update: UpdateState::Idle,
            show_toolbar: true,
            show_palette: true,
            fullscreen: false,
            screenshot: screenshot.map(|p| (p, 0)),
            shot: None,
            player: std::env::var("NAPKINUML_PLAY").ok().map(|s| Player::new(&s)),
            font_defs: egui::FontDefinitions::default(),
            export_dlg: None,
            batch: None,
            print_dlg: None,
            printing: None,
            job_cancel: false,
            seq_menu: None,
            sel_bar: None,
            alt_menu: None,
            conn_pick: None,
            lane_menu: None,
            ppp: 1.0,
            status: String::new(),
            canvas_rect: Rect::NOTHING,
            prop_session: None,
            fit_request: false,
            allow_close: false,
            title: String::new(),
            menu: None,
            menu_top_rects: [Rect::NOTHING; 5],
            nav_by_type: false,
            current_folder: None,
            label_rects: vec![],
            rename_focused: None,
            tree_open: HashMap::new(),
            tree_sel: None,
            tree_focus: false,
            tree_rows: vec![],
            tree_reveal: false,
            tree_last_click: None,
            font_loaded: vec![],
            font_pending: vec![],
            fonts_ready: false,
            fonts_check: true,
        };
        app.apply_theme(&cc.egui_ctx);
        if let Some(p) = open {
            app.open_path(p);
        }
        let _ = empty;
        if app.project.is_none() {
            // a normal start opens a new project; a project file given on the command line
            // (double-clicking a .umlp file) was opened above
            if start_diagram.is_none() {
                app.new_project();
            } else {
                app.project = Some(Project::sample());
                let ds = &app.project.as_ref().unwrap().diagrams;
                let first = ds[start_diagram.unwrap_or(0).min(ds.len() - 1)].id;
                app.open_diagram(first);
                app.dirty = false;
            }
        }
        app.status = app.t("Ready").to_string();
        if app.screenshot.is_none() && !cfg!(debug_assertions) {
            // double-clicking a .umlp file opens it in this program (per-user registration);
            // only the finished (release) program registers, so a test build never takes over
            crate::assoc::register_file_type();
        }
        app.refresh_fonts(&cc.egui_ctx);
        // for checking the folder name dialog in pictures: NAPKINUML_NEW_FOLDER=1
        if std::env::var("NAPKINUML_NEW_FOLDER").is_ok() {
            app.apply_nav(NavAction::NewFolder(None));
        }
        // for checking the AI Assistant in pictures: NAPKINUML_AI_PANEL=1, and
        // NAPKINUML_AI_REPLY=<file> applies a saved agent answer as if it had just arrived
        if std::env::var("NAPKINUML_AI_PANEL").is_ok() {
            app.show_ai = true;
        }
        if let Ok(f) = std::env::var("NAPKINUML_AI_REPLY") {
            if let Ok(text) = std::fs::read_to_string(f) {
                app.show_ai = true;
                app.ai_chat.push((true, "(test answer)".into()));
                app.ai_answer(Ok(text));
            }
        }
        // for checking the hidden left pane in pictures: NAPKINUML_HIDE_LEFT=1
        if std::env::var("NAPKINUML_HIDE_PROPERTY").is_ok() {
            app.show_property = false;
        }
        // for checking the About dialog in pictures: NAPKINUML_ABOUT=1
        if std::env::var("NAPKINUML_ABOUT").is_ok() {
            app.about_open = true;
        }
        // for checking a top menu in pictures: NAPKINUML_MENU=<index> opens it with its first row lit
        if let Some(top) = std::env::var("NAPKINUML_MENU").ok().and_then(|v| v.parse::<usize>().ok()) {
            app.menu = Some(MenuState { top, path: vec![0] });
        }
        if std::env::var("NAPKINUML_HIDE_LEFT").is_ok() {
            app.show_left = false;
        }
        // for checking ball/socket joining in pictures: NAPKINUML_JOIN=<shape id> acts as if that
        // shape had just been dropped where it is
        if let Some(id) = std::env::var("NAPKINUML_JOIN").ok().and_then(|v| v.parse::<u64>().ok()) {
            app.join_interfaces(&[id]);
        }
        // for checking the Print dialog in pictures: NAPKINUML_PRINT_DLG=1
        if std::env::var("NAPKINUML_PRINT_DLG").is_ok() {
            app.open_print_dialog();
            if let (Ok(s), Some(d)) = (std::env::var("NAPKINUML_PICK_SEARCH"), app.print_dlg.as_mut()) {
                d.pk.search = s;
            }
        }
        // for checking the multi-image export dialog in pictures: NAPKINUML_EXPORT_DLG=1
        if std::env::var("NAPKINUML_EXPORT_DLG").is_ok() {
            app.open_export_dialog();
        }
        // for checking the property pane in pictures: NAPKINUML_SELECT_FIRST=1
        if std::env::var("NAPKINUML_SELECT_FIRST").is_ok() {
            if let Some(id) = app.diag().and_then(|d| d.shapes.first()).map(|s| s.id) {
                app.sel_shapes = vec![id];
            }
        }
        // for checking the menus in pictures: NAPKINUML_OPEN_MENU=<menu>[,<row>[,<row>]]
        if let Ok(v) = std::env::var("NAPKINUML_OPEN_MENU") {
            let n: Vec<usize> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if let Some(&top) = n.first() {
                let path = if n.len() > 1 { n[1..].to_vec() } else { vec![0] };
                app.menu = Some(MenuState { top: top.min(3), path });
            }
        }
        // look for a newer version in the background (only the finished program, only the
        // first window; NAPKINUML_UPDATE_TEST=1 checks from a test build too)
        if app.screenshot.is_none() {
            crate::update::cleanup_old();
            let wanted = !cfg!(debug_assertions) || std::env::var("NAPKINUML_UPDATE_TEST").is_ok();
            if wanted && app.settings.check_updates && std::env::var_os(CHILD_WINDOW_ENV).is_none() {
                app.update = UpdateState::Checking(crate::update::check(cc.egui_ctx.clone()));
            }
        }
        app
    }

    fn t<'a>(&self, s: &'a str) -> &'a str {
        tr(self.settings.language, s)
    }

    /// The window's colours. The theme is pinned to light or dark (not "follow the system"), so
    /// the same colours apply from the first frame on and stay the same when options are applied
    /// (before, the system theme replaced them at start and a darker blue appeared after OK).
    fn apply_theme(&self, ctx: &egui::Context) {
        let theme = if self.settings.dark { egui::Theme::Dark } else { egui::Theme::Light };
        let mut v = if self.settings.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        if !self.settings.dark {
            v.extreme_bg_color = Color32::WHITE;
        }
        // selections keep egui's own soft blue (light: 144,209,255)
        ctx.set_theme(theme);
        ctx.set_visuals_of(theme, v);
    }

    fn look(&self) -> Look {
        let o = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        // the project's font wins over the application's
        let font = if !o.font.is_empty() { o.font.clone() } else { self.settings.font.clone() };
        let family = if font.is_empty() { None } else { render::family_ref(&font) };
        let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
        Look {
            dark: self.settings.dark,
            text_size: self.settings.text_size,
            icon: false,
            family,
            bg: rgb(o.diagram_background),
            ink: rgb(o.text_color),
            shape_fill: rgb(o.shape_fill),
        }
    }

    fn diag(&self) -> Option<&Diagram> {
        let id = self.active?;
        self.project.as_ref()?.diagram(id)
    }

    fn diag_mut(&mut self) -> Option<&mut Diagram> {
        let id = self.active?;
        self.project.as_mut()?.diagram_mut(id)
    }

    fn view(&self) -> ViewState {
        self.active
            .and_then(|id| self.views.get(&id).copied())
            .unwrap_or(ViewState { pan: Vec2::new(20.0, 20.0), zoom: 1.0, show_grid: self.settings.show_grid })
    }

    fn view_mut(&mut self) -> Option<&mut ViewState> {
        let id = self.active?;
        let default = ViewState { pan: Vec2::new(20.0, 20.0), zoom: 1.0, show_grid: self.settings.show_grid };
        Some(self.views.entry(id).or_insert(default))
    }

    fn clear_selection(&mut self) {
        self.sel_bar = None;
        self.sel_shapes.clear();
        self.sel_conns.clear();
        self.prop_session = None;
    }

    /// Load the fonts the settings and the project use; egui can draw with them next frame.
    fn refresh_fonts(&mut self, ctx: &egui::Context) {
        self.fonts_check = false;
        let mut names: Vec<String> = vec![self.settings.font.clone()];
        if let Some(p) = &self.project {
            names.push(p.options.font.clone());
            for d in &p.diagrams {
                for s in &d.shapes {
                    if let Some(f) = &s.font {
                        names.push(f.clone());
                    }
                }
            }
        }
        let mut wanted = self.font_loaded.clone();
        for n in names {
            if !n.is_empty() && crate::fonts::available().contains(&n.as_str()) && !wanted.contains(&n) {
                wanted.push(n);
            }
        }
        if self.fonts_ready && wanted.len() == self.font_loaded.len() {
            return;
        }
        let (defs, loaded) = crate::fonts::build(&wanted);
        self.font_defs = defs.clone();
        ctx.set_fonts(defs);
        self.fonts_ready = true;
        self.font_pending = loaded.clone();
        self.font_loaded = loaded;
        ctx.request_repaint();
    }
}

impl eframe::App for UmlApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.play_input(ctx, raw_input);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ppp = ctx.pixels_per_point();
        self.work_jobs(ctx);
        self.update_poll();
        // fonts given to egui last frame are usable now
        for n in self.font_pending.drain(..) {
            render::register_family(&n);
        }
        if self.fonts_check {
            self.refresh_fonts(ctx);
        }
        self.handle_screenshots(ctx);

        // closing the window with unsaved changes asks first
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.dirty && self.screenshot.is_none() && self.project.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Exit);
        }
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        let title = format!(
            "{}{} - NapkinUML",
            self.project.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "No project".into()),
            if self.dirty { " *" } else { "" }
        );
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        self.shortcuts(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ui));
        if self.show_toolbar {
            egui::TopBottomPanel::top("toolbar").show(ctx, |ui| self.toolbar(ui));
        }
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(d) = self.diag() {
                        let lang = self.settings.language;
                        ui.label(format!("{}%", (self.view().zoom * 100.0).round()));
                        ui.separator();
                        ui.label(format!("{} {} · {} {}", d.shapes.len(), tr(lang, "elements"), d.connectors.len(), tr(lang, "connectors")));
                        ui.separator();
                        ui.label(tr(lang, d.kind.label()));
                    }
                });
            });
        });
        if !self.show_left {
            let lang = self.settings.language;
            egui::SidePanel::left("left_strip").resizable(false).exact_width(22.0).show(ctx, |ui| {
                ui.add_space(2.0);
                if ui.add(egui::Button::new("»").frame(false)).on_hover_text(tr(lang, "Show Diagram Navigator and Model Explorer")).clicked() {
                    self.show_left = true;
                }
            });
        }
        if self.show_left {
            egui::SidePanel::left("left").default_width(225.0).min_width(160.0).show(ctx, |ui| self.left_pane(ui));
        }
        self.ai_poll(ctx);
        if self.show_ai {
            egui::SidePanel::right("ai").default_width(320.0).min_width(240.0).show(ctx, |ui| self.ai_pane(ui));
        }
        if !self.show_property {
            // a thin strip keeps the hidden Property pane one click away
            let lang = self.settings.language;
            egui::SidePanel::right("property_strip").resizable(false).exact_width(22.0).show(ctx, |ui| {
                ui.add_space(2.0);
                if ui.add(egui::Button::new("«").frame(false)).on_hover_text(tr(lang, "Show Property")).clicked() {
                    self.show_property = true;
                }
            });
        }
        if self.show_property {
            egui::SidePanel::right("property").default_width(250.0).min_width(200.0).show(ctx, |ui| self.property_pane(ui));
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            if !self.tabs.is_empty() {
                egui::TopBottomPanel::top("tabs").show_inside(ui, |ui| self.tab_bar(ui));
            }
            if let Some(kind) = self.diag().map(|d| d.kind) {
                if self.show_palette {
                    let w = if self.settings.palette_labels { 172.0 } else { 34.0 };
                    egui::SidePanel::left("palette").resizable(false).exact_width(w).show_inside(ui, |ui| self.palette_pane(ui, kind));
                }
            }
            egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| self.canvas(ui));
        });

        self.dialogs(ctx);
        self.menu_popups(ctx);

        if !matches!(self.drag, Drag::None) {
            ctx.request_repaint();
        }
    }
}

impl UmlApp {
    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut close: Option<u64> = None;
                let mut open: Option<u64> = None;
                let mut rename: Option<(u64, String)> = None;
                for id in self.tabs.clone() {
                    let Some(d) = self.project.as_ref().and_then(|p| p.diagram(id)) else { continue };
                    let name = d.name.clone();
                    let active = self.active == Some(id);
                    egui::Frame::NONE
                        .fill(if active { ui.visuals().extreme_bg_color } else { Color32::TRANSPARENT })
                        .corner_radius(3.0)
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let r = ui.selectable_label(active, &name).on_hover_text(tr(self.settings.language, "Double-click to rename"));
                                if r.clicked() {
                                    open = Some(id);
                                }
                                if r.double_clicked() {
                                    rename = Some((id, name.clone()));
                                }
                                if ui.small_button("×").clicked() {
                                    close = Some(id);
                                }
                            });
                        });
                }
                if let Some(id) = open {
                    self.open_diagram(id);
                }
                if let Some(id) = close {
                    self.close_diagram(id);
                }
                // a double-click on a tab opens Rename for that diagram
                if rename.is_some() {
                    self.rename = rename;
                }
            });
        });
    }
}
