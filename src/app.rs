//! The main window: menus, toolbar, panes, canvas and dialogs.

use crate::i18n::{tr, Language};
use crate::model::{ConnKind, Connector, Diagram, DiagramKind, ErdNotation, Folder, Project, ProjectOptions, Shape, ShapeKind};
use crate::palette::{self, Tool};
use crate::render::{self, Cam, Look};
use crate::settings::Settings;
use egui::{Align2, Color32, Key, KeyboardShortcut, Modifiers, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::HashMap;
use std::path::PathBuf;

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

/// Where the update offered at start stands.
enum UpdateState {
    Idle,
    Checking(std::sync::mpsc::Receiver<Option<crate::update::Release>>),
    Available(crate::update::Release),
    Downloading(crate::update::Job, crate::update::Release),
    /// Installed: the program to start for the new version.
    Ready(PathBuf, String),
    /// Updating failed: the message and the release page.
    Failed(String, String),
}

/// Set on windows this program opens itself (Window > New Window...), so only the first
/// window checks for updates.
const CHILD_WINDOW_ENV: &str = "NAPKINUML_CHILD_WINDOW";

enum Shot {
    Check(PathBuf),
}

#[derive(Clone, Copy, PartialEq)]
enum EditTarget {
    Shape(u64),
    Conn(u64),
}

struct InlineEdit {
    target: EditTarget,
    text: String,
    focus: bool,
    /// Editing one attribute (false) or operation (true) line of a shape, by index.
    line: Option<(bool, usize)>,
    /// The line was added for this edit (removed again if left empty).
    new_line: bool,
    /// Editing the name of one partition of a swimlane, by index.
    part: Option<usize>,
}

/// A handle of the selected line: a bend point, or the middle dot of a segment.
enum PointHit {
    Point(usize),
    Mid(usize),
}

/// Everything the menus can do.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Act {
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
}

/// One row of a menu.
enum MItem {
    Act { label: String, shortcut: String, enabled: bool, act: Act, checked: Option<bool> },
    Sub { label: String, enabled: bool, items: Vec<MItem> },
    Sep,
}

/// The open menu: which top menu, and the highlighted row at each open level.
#[derive(Clone)]
struct MenuState {
    top: usize,
    path: Vec<usize>,
}

/// A menu title with its access key underlined ("Project", or "프로젝트(P)").
fn mnemonic_job(label: &str, key: char, color: Color32, font: egui::FontId) -> egui::text::LayoutJob {
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

/// A name label on the canvas that can be dragged.
#[derive(Clone, Copy, PartialEq, Debug)]
enum LabelTarget {
    Conn(u64),
    Shape(u64),
    /// The source end label of a line.
    ConnFrom(u64),
    /// The target end label of a line.
    ConnTo(u64),
}

impl LabelTarget {
    fn owner(self) -> EditTarget {
        match self {
            LabelTarget::Conn(id) | LabelTarget::ConnFrom(id) | LabelTarget::ConnTo(id) => EditTarget::Conn(id),
            LabelTarget::Shape(id) => EditTarget::Shape(id),
        }
    }
}

/// When a line is drawn between two side-by-side (or stacked) boxes and the drag starts
/// away from the middle of the box, the line runs straight across at that height (or
/// position): both ends are fixed there. Starting near the middle leaves the ends free.
fn press_lane(a: &Shape, b: &Shape, press: (f32, f32)) -> Option<((f32, f32), (f32, f32))> {
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
fn takes_ports(k: ShapeKind) -> bool {
    matches!(k, ShapeKind::Component | ShapeKind::Node | ShapeKind::Device | ShapeKind::ExecutionEnvironment | ShapeKind::Class | ShapeKind::Instance)
}

/// Ports sitting on (or inside) a shape's outline: their centre is within half a port of it.
fn attached_ports(d: &Diagram, owner: &Shape) -> Vec<u64> {
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

fn is_interface_end(k: ShapeKind) -> bool {
    matches!(k, ShapeKind::ProvidedInterface | ShapeKind::RequiredInterface)
}

fn opposite_interface(a: ShapeKind, b: ShapeKind) -> bool {
    is_interface_end(a) && is_interface_end(b) && a != b
}

/// A provided interface (ball) and a required interface (socket) whose centres sit on each
/// other are joined: the ball rests in the socket and they move as one.
fn joined_interfaces(d: &Diagram, s: &Shape) -> Vec<u64> {
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
fn port_owner(d: &Diagram, port: &Shape) -> Option<u64> {
    let (cx, cy) = port.center();
    d.shapes
        .iter()
        .filter(|s| takes_ports(s.kind) && s.id != port.id)
        .filter(|s| cx >= s.x - 12.0 && cx <= s.x + s.w + 12.0 && cy >= s.y - 12.0 && cy <= s.y + s.h + 12.0)
        .min_by(|a, b| (a.w * a.h).partial_cmp(&(b.w * b.h)).unwrap_or(std::cmp::Ordering::Equal))
        .map(|s| s.id)
}

/// Put a port's centre on the nearest edge of its owner.
fn snap_port(port: &mut Shape, owner: &Shape) {
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

/// Folders nest at most this many levels.
const MAX_FOLDER_DEPTH: usize = 3;

/// Something dragged in the Diagram Navigator.
#[derive(Clone, Copy, Debug)]
enum NavDrag {
    Diagram(u64),
    Folder(u64),
}

enum NavAction {
    NewFolder(Option<u64>),
    NewDiagram(DiagramKind, Option<u64>),
    RenameFolder(u64),
    DeleteFolder(u64),
    Move(NavDrag, Option<u64>),
}

/// How many folders deep a folder is (the project root is 0, a top folder 1).
fn folder_level(p: &Project, f: Option<u64>) -> usize {
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
fn folder_height(p: &Project, id: u64) -> usize {
    1 + p.folders.iter().filter(|f| f.parent == Some(id)).map(|f| folder_height(p, f.id)).max().unwrap_or(0)
}

/// Is folder `t` the folder `outer` or inside it?
fn is_inside(p: &Project, t: u64, outer: u64) -> bool {
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
fn folder_paths(p: &Project) -> Vec<(u64, String)> {
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

/// Twelve pastel fill colours offered as presets.
const PASTELS: [(&str, [u8; 3]); 18] = [
    ("Pink", [255, 209, 220]),
    ("Coral", [255, 183, 178]),
    ("Peach", [255, 218, 185]),
    ("Apricot", [255, 232, 204]),
    ("Butter", [255, 247, 174]),
    ("Lime", [226, 240, 203]),
    ("Mint", [189, 236, 182]),
    ("Aqua", [181, 234, 215]),
    ("Teal", [178, 222, 224]),
    ("Sky", [186, 225, 255]),
    ("Powder Blue", [214, 230, 245]),
    ("Periwinkle", [199, 206, 234]),
    ("Lavender", [224, 187, 228]),
    ("Lilac", [242, 214, 246]),
    ("Rose", [246, 200, 226]),
    ("Sand", [237, 228, 211]),
    ("Stone", [222, 214, 206]),
    ("Silver", [226, 229, 234]),
];

/// A row of pastel swatches; returns the colour clicked.
fn pastel_swatches(ui: &mut egui::Ui, lang: Language, current: Option<[u8; 4]>) -> Option<[u8; 4]> {
    let mut pick = None;
    let accent = ui.visuals().selection.bg_fill;
    let edge = ui.visuals().widgets.noninteractive.bg_stroke.color;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        for (name, c) in PASTELS {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
            let chosen = current.map(|f| f[0] == c[0] && f[1] == c[1] && f[2] == c[2]).unwrap_or(false);
            let stroke = if chosen { Stroke::new(2.0_f32, accent) } else { Stroke::new(1.0_f32, edge) };
            ui.painter().rect(rect, 3.0, Color32::from_rgb(c[0], c[1], c[2]), stroke, StrokeKind::Inside);
            let resp = resp.on_hover_text(tr(lang, name));
            if resp.clicked() {
                pick = Some([c[0], c[1], c[2], 255]);
            }
        }
    });
    pick
}

/// Lines that can carry navigability arrows.
fn navigable(kind: ConnKind) -> bool {
    matches!(kind, ConnKind::Association | ConnKind::Aggregation | ConnKind::Composition)
}

/// The "export several diagrams" dialog: which diagrams, and the folder to write them to.
struct ExportDlg {
    chosen: Vec<(u64, bool)>,
    folder: Option<PathBuf>,
    pk: Picker,
    format: ImgFormat,
}

/// The picture formats Export can write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ImgFormat {
    Png,
    Jpeg,
    Webp,
}

impl ImgFormat {
    const ALL: [ImgFormat; 3] = [ImgFormat::Png, ImgFormat::Jpeg, ImgFormat::Webp];
    fn ext(self) -> &'static str {
        match self {
            ImgFormat::Png => "png",
            ImgFormat::Jpeg => "jpg",
            ImgFormat::Webp => "webp",
        }
    }
    fn label(self) -> &'static str {
        match self {
            ImgFormat::Png => "PNG",
            ImgFormat::Jpeg => "JPEG",
            ImgFormat::Webp => "WebP",
        }
    }
    /// What the format is good for, shown beside the choice.
    fn note(self) -> &'static str {
        match self {
            ImgFormat::Png => "Sharp, no loss (best for diagrams)",
            ImgFormat::Jpeg => "Small files, slight blur on lines (quality 92)",
            ImgFormat::Webp => "Sharp, no loss, smaller than PNG",
        }
    }
    fn exts(self) -> &'static [&'static str] {
        match self {
            ImgFormat::Png => &["png"],
            ImgFormat::Jpeg => &["jpg", "jpeg"],
            ImgFormat::Webp => &["webp"],
        }
    }
    fn from_id(s: &str) -> ImgFormat {
        match s.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => ImgFormat::Jpeg,
            "webp" => ImgFormat::Webp,
            _ => ImgFormat::Png,
        }
    }
    fn from_path(p: &std::path::Path) -> Option<ImgFormat> {
        let e = p.extension()?.to_str()?.to_ascii_lowercase();
        ImgFormat::ALL.into_iter().find(|f| f.exts().contains(&e.as_str()))
    }
}

/// Search text and type filter of the diagram picker (Print and Export dialogs).
#[derive(Default)]
struct Picker {
    search: String,
    kind: Option<DiagramKind>,
}

/// Diagrams under a folder (and its subfolders) that pass the picker's filters; None = everything.
fn picked_under(p: &Project, folder: Option<u64>, visible: &std::collections::HashSet<u64>) -> Vec<u64> {
    let mut out: Vec<u64> = p.diagrams.iter().filter(|d| visible.contains(&d.id) && d.folder.filter(|f| p.folder(*f).is_some()) == folder).map(|d| d.id).collect();
    for sub in p.folders.iter().filter(|x| x.parent == folder) {
        out.extend(picked_under(p, Some(sub.id), visible));
    }
    out
}

/// One folder of the picker: a check box for everything inside it (a dash when only some
/// are ticked), its subfolders, then its diagrams. The root (None) has no header.
#[allow(clippy::too_many_arguments)]
fn picker_folder(
    ui: &mut egui::Ui,
    salt: &str,
    lang: Language,
    p: &Project,
    folder: Option<u64>,
    visible: &std::collections::HashSet<u64>,
    sel: &HashMap<u64, bool>,
    actions: &mut Vec<(Vec<u64>, bool)>,
    filtering: bool,
) {
    let mut subs: Vec<&Folder> = p.folders.iter().filter(|x| x.parent == folder).collect();
    subs.sort_by_key(|x| x.name.to_lowercase());
    let mut diagrams: Vec<&Diagram> = p.diagrams.iter().filter(|d| visible.contains(&d.id) && d.folder.filter(|f| p.folder(*f).is_some()) == folder).collect();
    diagrams.sort_by_key(|d| d.name.to_lowercase());
    let contents = |ui: &mut egui::Ui, actions: &mut Vec<(Vec<u64>, bool)>| {
        for sub in &subs {
            let ids = picked_under(p, Some(sub.id), visible);
            if ids.is_empty() {
                continue;
            }
            let all = ids.iter().all(|i| sel.get(i).copied().unwrap_or(false));
            let any = ids.iter().any(|i| sel.get(i).copied().unwrap_or(false));
            let id = ui.make_persistent_id((salt, sub.id));
            // folders start folded (an overview of a big project), except those holding a ticked diagram
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, any);
            if filtering {
                state.set_open(true);
            }
            state
                .show_header(ui, |ui| {
                    let mut v = all;
                    let ticked = ids.iter().filter(|i| sel.get(i).copied().unwrap_or(false)).count();
                    let label = format!("🗀 {}  ({ticked}/{})", sub.name, ids.len());
                    if ui.add(egui::Checkbox::new(&mut v, label).indeterminate(any && !all)).clicked() {
                        actions.push((ids.clone(), !all));
                    }
                })
                .body(|ui| picker_folder(ui, salt, lang, p, Some(sub.id), visible, sel, actions, filtering));
        }
        for d in &diagrams {
            let mut v = sel.get(&d.id).copied().unwrap_or(false);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut v, d.name.as_str()).changed() {
                    actions.push((vec![d.id], v));
                }
                ui.weak(tr(lang, d.kind.label()));
            });
        }
    };
    contents(ui, actions);
}

/// The diagram chooser of the Print and Export dialogs, made for big projects: a search box
/// (name or folder), a diagram-type filter, the diagrams grouped by folder with a check box per
/// folder, buttons for what is shown, and a count of what is ticked.
#[allow(clippy::too_many_arguments)]
fn diagram_picker(ui: &mut egui::Ui, salt: &str, lang: Language, p: &Project, chosen: &mut [(u64, bool)], pk: &mut Picker, open_tabs: &[u64], current: Option<u64>, height: f32) {
    let t = |s: &str| tr(lang, s).to_string();
    let paths: HashMap<u64, String> = folder_paths(p).into_iter().collect();
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut pk.search).hint_text(t("Search diagrams...")).desired_width(200.0));
        if !pk.search.is_empty() && ui.small_button("×").on_hover_text(t("Clear")).clicked() {
            pk.search.clear();
        }
        let mut kinds: Vec<DiagramKind> = DiagramKind::ALL.into_iter().filter(|k| p.diagrams.iter().any(|d| d.kind == *k)).collect();
        kinds.dedup();
        let shown = pk.kind.map(|k| t(k.label())).unwrap_or_else(|| t("All types"));
        egui::ComboBox::from_id_salt((salt, "kind")).width(170.0).selected_text(shown).show_ui(ui, |ui| {
            ui.selectable_value(&mut pk.kind, None, t("All types"));
            for k in kinds {
                ui.selectable_value(&mut pk.kind, Some(k), t(k.label()));
            }
        });
    });
    let needle = pk.search.trim().to_lowercase();
    let visible: std::collections::HashSet<u64> = p
        .diagrams
        .iter()
        .filter(|d| pk.kind.map(|k| d.kind == k).unwrap_or(true))
        .filter(|d| needle.is_empty() || d.name.to_lowercase().contains(&needle) || d.folder.and_then(|f| paths.get(&f)).map(|x| x.to_lowercase().contains(&needle)).unwrap_or(false))
        .map(|d| d.id)
        .collect();
    let filtering = !needle.is_empty() || pk.kind.is_some();
    let sel: HashMap<u64, bool> = chosen.iter().copied().collect();
    let mut actions: Vec<(Vec<u64>, bool)> = vec![];
    ui.horizontal(|ui| {
        let shown_ids: Vec<u64> = visible.iter().copied().collect();
        if ui.small_button(t(if filtering { "Tick shown" } else { "Select All" })).clicked() {
            actions.push((shown_ids.clone(), true));
        }
        if ui.small_button(t(if filtering { "Untick shown" } else { "Select None" })).clicked() {
            actions.push((shown_ids, false));
        }
        if let Some(cur) = current {
            if ui.small_button(t("Current tab only")).on_hover_text(t("Tick only the diagram shown in the editor now")).clicked() {
                actions.push((p.diagrams.iter().map(|d| d.id).collect(), false));
                actions.push((vec![cur], true));
            }
        }
        if !open_tabs.is_empty() && ui.small_button(t("Open tabs only")).on_hover_text(t("Tick exactly the diagrams open in tabs")).clicked() {
            actions.push((p.diagrams.iter().map(|d| d.id).collect(), false));
            actions.push((open_tabs.to_vec(), true));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let n = chosen.iter().filter(|c| c.1).count();
            ui.label(format!("{} {n} / {}", t("Selected"), chosen.len()));
        });
    });
    egui::Frame::NONE.stroke(ui.visuals().widgets.noninteractive.bg_stroke).inner_margin(4.0).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt((salt, "list")).max_height(height).min_scrolled_height(height).auto_shrink([false, false]).show(ui, |ui| {
            if visible.is_empty() {
                ui.weak(t("No diagram matches."));
            } else {
                picker_folder(ui, salt, lang, p, None, &visible, &sel, &mut actions, filtering);
            }
        });
    });
    for (ids, on) in actions {
        for c in chosen.iter_mut() {
            if ids.contains(&c.0) {
                c.1 = on;
            }
        }
    }
}

/// Diagrams being exported, one per frame, each drawn off screen (the window doesn't change).
struct BatchExport {
    queue: std::collections::VecDeque<(u64, PathBuf)>,
    total: usize,
    done: usize,
    failed: usize,
    folder: PathBuf,
    /// Frames to wait before working, so the progress window is on screen first.
    wait: u8,
}

/// What to do with the pages once every diagram has been drawn.
#[derive(Clone)]
enum PrintTo {
    Printer,
    Pdf(PathBuf),
}

/// The Print dialog: paper, orientation and the diagrams to print (one per page).
struct PrintDlg {
    paper: crate::print::Paper,
    orient: crate::print::Orient,
    chosen: Vec<(u64, bool)>,
    pk: Picker,
}

/// Printing in progress: the pages are drawn off screen, one per frame.
struct PrintJob {
    to: PrintTo,
    paper: crate::print::Paper,
    orient: crate::print::Orient,
    queue: std::collections::VecDeque<u64>,
    total: usize,
    pages: Vec<image::RgbImage>,
    /// Frames to wait before the next step, so the progress window shows what comes next.
    wait: u8,
    /// Every page is drawn; the printer dialog (or the PDF) comes next.
    finishing: bool,
}

/// A diagram name made safe as a file name.
fn file_name_for(name: &str) -> String {
    let mut s: String = name.chars().map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c }).collect();
    s = s.trim().trim_end_matches('.').to_string();
    if s.is_empty() {
        s = "diagram".into();
    }
    s
}

struct AppOptionsDlg {
    s: Settings,
    page: usize,
}

struct ProjectOptionsDlg {
    o: ProjectOptions,
    name: String,
    author: String,
    description: String,
    page: usize,
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
    menu_top_rects: [Rect; 4],
    nav_by_type: bool,
    current_folder: Option<u64>,
    label_rects: Vec<(LabelTarget, Rect)>,
    rename_focused: Option<u64>,
    font_loaded: Vec<String>,
    font_pending: Vec<String>,
    fonts_ready: bool,
    fonts_check: bool,
}

fn sc(mods: Modifiers, key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(mods, key)
}

const SC_NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const SC_OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SC_PRINT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::P);
const SC_COPY_IMAGE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::C);
const SC_SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SC_SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
const SC_UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const SC_REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const SC_SELECT_ALL: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::A);
const SC_DUPLICATE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
const SC_NEW_DIAGRAM: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::D);
const SC_ZOOM_IN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Equals);
const SC_ZOOM_OUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Minus);
const SC_ZOOM_100: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0);
const SC_PROPERTY: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::P);
const SC_FULLSCREEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F11);
const SC_SWITCHER: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::Tab);
const SC_NEW_WINDOW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::N);
const SC_CLOSE_DIAGRAM: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::W);

/// Make the dialog being drawn modal: Tab and Shift+Tab move the keyboard focus through its
/// boxes and buttons only (Enter or Space presses a focused button), and the window behind
/// it can't be clicked. Takes effect from the next frame.
fn modal_dialog(ui: &egui::Ui) {
    let layer = ui.layer_id();
    ui.ctx().memory_mut(|m| m.set_modal_layer(layer));
}

/// Esc pressed while no drop-down list or colour picker is open (Esc closes those first).
fn esc_cancels(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.key_pressed(Key::Escape)) && !ctx.memory(|m| m.any_popup_open())
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
            font_defs: egui::FontDefinitions::default(),
            export_dlg: None,
            batch: None,
            print_dlg: None,
            printing: None,
            job_cancel: false,
            seq_menu: None,
            sel_bar: None,
            alt_menu: None,
            lane_menu: None,
            ppp: 1.0,
            status: String::new(),
            canvas_rect: Rect::NOTHING,
            prop_session: None,
            fit_request: false,
            allow_close: false,
            title: String::new(),
            menu: None,
            menu_top_rects: [Rect::NOTHING; 4],
            nav_by_type: false,
            current_folder: None,
            label_rects: vec![],
            rename_focused: None,
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

    // ---------- project & diagram management ----------

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

    fn open_diagram(&mut self, id: u64) {
        if !self.tabs.contains(&id) {
            self.tabs.push(id);
        }
        if self.active != Some(id) {
            self.active = Some(id);
            self.clear_selection();
            self.tool = Tool::Select;
            self.editing = None;
        }
    }

    fn close_diagram(&mut self, id: u64) {
        let idx = self.tabs.iter().position(|t| *t == id);
        self.tabs.retain(|t| *t != id);
        if self.active == Some(id) {
            self.active = idx.and_then(|i| self.tabs.get(i.min(self.tabs.len().saturating_sub(1))).copied()).or(self.tabs.last().copied());
            self.clear_selection();
            self.editing = None;
        }
    }

    fn new_project(&mut self) {
        self.project = Some(Project::new("Untitled"));
        self.path = None;
        self.dirty = false;
        self.tabs.clear();
        self.active = None;
        self.views.clear();
        self.current_folder = None;
        self.undo.clear();
        self.redo.clear();
        self.clear_selection();
        // a new project opens with a class diagram ready to draw on
        let id = self.project.as_mut().unwrap().add_diagram(DiagramKind::Class, None);
        self.open_diagram(id);
        self.dirty = false;
    }

    fn add_diagram(&mut self, kind: DiagramKind, name: Option<String>) {
        if self.project.is_none() {
            self.new_project();
        }
        let folder = self.current_folder;
        let p = self.project.as_mut().unwrap();
        let id = p.add_diagram(kind, name);
        // a new diagram goes into the folder last chosen in the Diagram Navigator
        if let Some(f) = folder.filter(|f| p.folder(*f).is_some()) {
            if let Some(d) = p.diagram_mut(id) {
                d.folder = Some(f);
            }
        }
        self.open_diagram(id);
        self.dirty = true;
    }

    fn delete_diagram(&mut self, id: u64) {
        self.close_diagram(id);
        if let Some(p) = self.project.as_mut() {
            p.diagrams.retain(|d| d.id != id);
        }
        self.undo.retain(|(d, _)| *d != id);
        self.redo.retain(|(d, _)| *d != id);
        self.dirty = true;
    }

    fn duplicate_diagram(&mut self, id: u64) {
        let Some(p) = self.project.as_mut() else { return };
        let Some(src) = p.diagram(id).cloned() else { return };
        let mut map = HashMap::new();
        let nid = p.fresh_id();
        let mut d = src.clone();
        d.id = nid;
        d.name = format!("{} (copy)", src.name);
        for s in d.shapes.iter_mut() {
            let n = p.fresh_id();
            map.insert(s.id, n);
            s.id = n;
        }
        for c in d.connectors.iter_mut() {
            let n = p.fresh_id();
            map.insert(c.id, n);
            c.id = n;
        }
        for c in d.connectors.iter_mut() {
            c.from = map.get(&c.from).copied().unwrap_or(c.from);
            c.to = map.get(&c.to).copied().unwrap_or(c.to);
        }
        p.diagrams.push(d);
        self.open_diagram(nid);
        self.dirty = true;
    }

    fn save_to(&mut self, path: PathBuf) -> bool {
        let Some(p) = self.project.as_mut() else { return false };
        if p.name == "Untitled" {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                p.name = stem.to_string();
            }
        }
        match serde_json::to_string_pretty(p) {
            Ok(text) => match std::fs::write(&path, text) {
                Ok(()) => {
                    self.settings.add_recent(&path.to_string_lossy());
                    self.settings.save();
                    self.status = format!("Saved {}", path.display());
                    self.path = Some(path);
                    self.dirty = false;
                    true
                }
                Err(e) => {
                    self.status = format!("Could not save: {e}");
                    false
                }
            },
            Err(e) => {
                self.status = format!("Could not save: {e}");
                false
            }
        }
    }

    fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(p) => self.save_to(p),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) -> bool {
        let name = self.project.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "Untitled".into());
        let picked = rfd::FileDialog::new().add_filter("UML Project", &[FILE_EXT]).set_file_name(format!("{name}.{FILE_EXT}")).save_file();
        match picked {
            Some(mut p) => {
                if p.extension().is_none() {
                    p.set_extension(FILE_EXT);
                }
                self.save_to(p)
            }
            None => false,
        }
    }

    fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("UML Project", &[FILE_EXT, "json"]).pick_file() {
            self.open_path(p);
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        let loaded = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<Project>(&t).map_err(|e| e.to_string()));
        match loaded {
            Ok(mut p) => {
                p.repair_ids();
                let first = p.diagrams.first().map(|d| d.id);
                self.project = Some(p);
                self.tabs.clear();
                self.active = None;
                self.views.clear();
        self.current_folder = None;
                self.undo.clear();
                self.redo.clear();
                self.clear_selection();
                if let Some(id) = first {
                    self.open_diagram(id);
                }
                self.settings.add_recent(&path.to_string_lossy());
                self.settings.save();
                self.fonts_check = true;
                self.status = format!("Opened {}", path.display());
                self.path = Some(path);
                self.dirty = false;
            }
            Err(e) => {
                self.status = format!("Could not open {}: {e}", path.display());
                let s = path.to_string_lossy().to_string();
                self.settings.recent.retain(|r| *r != s);
            }
        }
    }

    fn close_project(&mut self) {
        self.project = None;
        self.path = None;
        self.dirty = false;
        self.tabs.clear();
        self.active = None;
        self.undo.clear();
        self.redo.clear();
        self.clear_selection();
    }

    /// Run an action that would discard the project, asking to save first when needed.
    fn request(&mut self, action: Pending) {
        if self.dirty && self.project.is_some() && self.screenshot.is_none() {
            self.confirm = Some(action);
        } else {
            self.perform(action);
        }
    }

    fn perform(&mut self, action: Pending) {
        match action {
            Pending::New => self.new_project(),
            Pending::Open => self.open_dialog(),
            Pending::OpenPath(p) => self.open_path(p),
            Pending::Close => self.close_project(),
            Pending::Example => self.open_example(),
            Pending::Exit => {
                self.allow_close = true;
            }
            Pending::Restart(target) => match crate::update::relaunch(&target, self.path.as_deref()) {
                Ok(()) => self.allow_close = true,
                Err(e) => self.status = format!("{}: {e}", self.t("Could not restart")),
            },
        }
    }

    /// The example project with one diagram of every type.
    fn open_example(&mut self) {
        self.project = Some(Project::sample());
        self.path = None;
        self.tabs.clear();
        self.active = None;
        self.views.clear();
        self.current_folder = None;
        self.undo.clear();
        self.redo.clear();
        self.clear_selection();
        if let Some(first) = self.project.as_ref().and_then(|p| p.diagrams.first()).map(|d| d.id) {
            self.open_diagram(first);
        }
        self.fonts_check = true;
        self.dirty = false;
    }

    /// Pick a project file and open it in a new window (this window keeps its project).
    fn open_in_new_window(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter("UML Project", &[FILE_EXT, "json"]).pick_file() else { return };
        match std::env::current_exe() {
            Ok(exe) => match std::process::Command::new(exe).arg(&p).env(CHILD_WINDOW_ENV, "1").spawn() {
                Ok(_) => {
                    self.settings.add_recent(&p.to_string_lossy());
                    self.settings.save();
                    self.status = format!("Opened {} in a new window", p.display());
                }
                Err(e) => self.status = format!("Could not open a new window: {e}"),
            },
            Err(e) => self.status = format!("Could not open a new window: {e}"),
        }
    }

    fn new_window(&mut self) {
        match std::env::current_exe() {
            Ok(exe) => match std::process::Command::new(exe).arg("--new").env(CHILD_WINDOW_ENV, "1").spawn() {
                Ok(_) => self.status = "Opened a new window".into(),
                Err(e) => self.status = format!("Could not open a new window: {e}"),
            },
            Err(e) => self.status = format!("Could not open a new window: {e}"),
        }
    }

    // ---------- AI Assistant ----------

    /// The agent chosen in the panel, if it can run.
    fn ai_agent(&self) -> Option<crate::ai::Agent> {
        if self.settings.ai_agent == "custom" {
            return Some(crate::ai::Agent::Custom(self.settings.ai_command.clone())).filter(|_| !self.settings.ai_command.trim().is_empty());
        }
        self.ai_agents
            .iter()
            .find(|f| f.id == self.settings.ai_agent)
            .or_else(|| self.ai_agents.first())
            .cloned()
            .map(crate::ai::Agent::Known)
    }

    fn ai_send(&mut self) {
        let request = self.ai_input.trim().to_string();
        if request.is_empty() || self.ai_job.is_some() {
            return;
        }
        let Some(agent) = self.ai_agent() else {
            self.ai_chat.push((false, tr(self.settings.language, "No AI agent is set. Install Claude Code, Codex CLI, Gemini CLI or Ollama, or set a custom command.").to_string()));
            return;
        };
        let prompt = crate::ai::build_prompt(self.project.as_ref(), self.diag(), &self.ai_chat, &request);
        self.ai_chat.push((true, request));
        self.ai_input.clear();
        self.ai_job = Some(crate::ai::start(agent, self.settings.ai_model.clone(), prompt));
    }

    // ---------- updates ----------

    /// Pick up the background update check and download.
    fn update_poll(&mut self) {
        match &self.update {
            UpdateState::Checking(rx) => match rx.try_recv() {
                Ok(Some(rel)) if rel.version != self.settings.skip_version => self.update = UpdateState::Available(rel),
                // no newer version, a skipped one, or the check failed (offline...): say nothing
                Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected) => self.update = UpdateState::Idle,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            },
            UpdateState::Downloading(job, rel) => match job.rx.try_recv() {
                Ok(Ok(target)) => self.update = UpdateState::Ready(target, rel.version.clone()),
                Ok(Err(_)) if job.cancel.load(std::sync::atomic::Ordering::Relaxed) => self.update = UpdateState::Idle,
                Ok(Err(e)) => self.update = UpdateState::Failed(e, rel.page.clone()),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.update = UpdateState::Failed("The download stopped unexpectedly.".into(), rel.page.clone()),
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            },
            _ => {}
        }
    }

    /// The update notices, in the lower right corner. They don't block the window: the
    /// person can keep working and answer when it suits them.
    fn update_dialogs(&mut self, ctx: &egui::Context) {
        if matches!(self.update, UpdateState::Idle | UpdateState::Checking(_)) {
            return;
        }
        let lang = self.settings.language;
        let t = |s: &'static str| tr(lang, s);
        let mut next: Option<UpdateState> = None;
        let mut restart: Option<PathBuf> = None;
        egui::Window::new(t("Update"))
            .id(egui::Id::new("update_notice"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::RIGHT_BOTTOM, [-12.0, -36.0])
            .show(ctx, |ui| {
                ui.set_max_width(340.0);
                match &self.update {
                    UpdateState::Available(rel) => {
                        ui.label(format!("{} {}", t("A new version of NapkinUML is available:"), rel.version));
                        ui.label(format!("{} {}", t("Current version:"), crate::update::CURRENT));
                        ui.hyperlink_to(t("Release notes"), &rel.page);
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(t("Update")).clicked() {
                                next = Some(UpdateState::Downloading(crate::update::download_and_install(rel, ctx.clone()), rel.clone()));
                            }
                            if ui.button(t("Later")).clicked() {
                                next = Some(UpdateState::Idle);
                            }
                            if ui.button(t("Skip This Version")).clicked() {
                                self.settings.skip_version = rel.version.clone();
                                self.settings.save();
                                next = Some(UpdateState::Idle);
                            }
                        });
                    }
                    UpdateState::Downloading(job, rel) => {
                        ui.label(format!("{} {}", t("Downloading version"), rel.version));
                        let done = job.done.load(std::sync::atomic::Ordering::Relaxed);
                        let bar = if job.total > 0 {
                            egui::ProgressBar::new(done as f32 / job.total as f32).show_percentage()
                        } else {
                            egui::ProgressBar::new(0.0).text(format!("{:.1} MB", done as f64 / 1_048_576.0)).animate(true)
                        };
                        ui.add(bar.desired_width(300.0));
                        ui.add_space(4.0);
                        if ui.button(t("Cancel")).clicked() {
                            job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                            next = Some(UpdateState::Idle);
                        }
                    }
                    UpdateState::Ready(target, version) => {
                        ui.label(format!("{} {}", t("NapkinUML has been updated to version"), version));
                        ui.label(t("The new version starts the next time NapkinUML is opened."));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(t("Restart Now")).clicked() {
                                restart = Some(target.clone());
                                next = Some(UpdateState::Idle);
                            }
                            if ui.button(t("Later")).clicked() {
                                next = Some(UpdateState::Idle);
                            }
                        });
                    }
                    UpdateState::Failed(msg, page) => {
                        ui.label(t("The update could not be installed."));
                        ui.label(egui::RichText::new(msg).weak());
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(t("Open Download Page")).clicked() {
                                crate::update::open_page(page);
                                next = Some(UpdateState::Idle);
                            }
                            if ui.button(t("Close")).clicked() {
                                next = Some(UpdateState::Idle);
                            }
                        });
                    }
                    UpdateState::Idle | UpdateState::Checking(_) => {}
                }
            });
        if let Some(s) = next {
            self.update = s;
        }
        if let Some(target) = restart {
            // unsaved changes are asked about first, as when closing
            self.request(Pending::Restart(target));
        }
    }

    fn ai_poll(&mut self, ctx: &egui::Context) {
        let Some(job) = self.ai_job.as_ref() else { return };
        match job.rx.try_recv() {
            Ok(result) => {
                self.ai_job = None;
                self.ai_answer(result);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(250)),
            Err(_) => {
                self.ai_job = None;
                self.ai_chat.push((false, "The AI agent stopped unexpectedly.".into()));
            }
        }
    }

    /// Show the agent's answer and put its diagram on the canvas (undo takes it back).
    fn ai_answer(&mut self, result: Result<String, String>) {
        let lang = self.settings.language;
        let text = match result {
            Ok(t) => t,
            Err(e) => {
                self.ai_chat.push((false, e));
                return;
            }
        };
        let reply = crate::ai::parse_reply(&text);
        let mut message = reply.message.clone();
        if let Some(d) = reply.diagram {
            if self.project.is_none() {
                self.new_project();
            }
            // the open diagram's state before the change goes on the undo list
            if self.active.is_some() {
                self.checkpoint();
            }
            let active = self.active;
            let result = crate::ai::apply(self.project.as_mut().unwrap(), active, &d);
            match result {
                Ok(done) => {
                    if done.created && active.is_some() {
                        // a new diagram: the open one did not change
                        self.undo.pop();
                    }
                    if done.created {
                        if let (Some(f), Some(p)) = (self.current_folder, self.project.as_mut()) {
                            if p.folder(f).is_some() {
                                if let Some(dm) = p.diagram_mut(done.diagram) {
                                    dm.folder = Some(f);
                                }
                            }
                        }
                        self.open_diagram(done.diagram);
                    } else if let Some((_, dm)) = self.undo.last() {
                        // nothing changed: don't keep an empty undo step
                        if self.diag().map(|x| x.shapes.len() == dm.shapes.len() && serde_json::to_string(x).ok() == serde_json::to_string(dm).ok()).unwrap_or(false) {
                            self.undo.pop();
                        }
                    }
                    self.clear_selection();
                    self.dirty = true;
                    self.fonts_check = true;
                    self.status = format!("AI: {} {} · {} {}", done.shapes, tr(lang, "elements"), done.connectors, tr(lang, "connectors"));
                    if message.is_empty() {
                        message = tr(lang, "The diagram was updated.").to_string();
                    }
                }
                Err(e) => {
                    if self.active.is_some() {
                        self.undo.pop();
                    }
                    message = if message.is_empty() { e } else { format!("{message}\n({e})") };
                }
            }
        }
        if message.is_empty() {
            message = text.trim().chars().take(2000).collect();
        }
        self.ai_chat.push((false, message));
    }

    fn ai_pane(&mut self, ui: &mut egui::Ui) {
        let lang = self.settings.language;
        let t = |s: &str| tr(lang, s).to_string();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading(t("AI Assistant"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("×").on_hover_text(t("Close")).clicked() {
                    self.show_ai = false;
                }
                if !self.ai_chat.is_empty() && self.ai_job.is_none() && ui.small_button(t("Clear")).clicked() {
                    self.ai_chat.clear();
                }
            });
        });
        ui.separator();
        // which agent
        let mut choice = self.settings.ai_agent.clone();
        if choice.is_empty() || (choice != "custom" && !self.ai_agents.iter().any(|f| f.id == choice)) {
            choice = self.ai_agents.first().map(|f| f.id.to_string()).unwrap_or_else(|| "custom".into());
        }
        let label = if choice == "custom" { t("Custom command") } else { self.ai_agents.iter().find(|f| f.id == choice).map(|f| f.label.to_string()).unwrap_or_default() };
        ui.horizontal(|ui| {
            ui.label(t("Agent"));
            egui::ComboBox::from_id_salt("ai_agent").selected_text(label).width(170.0).show_ui(ui, |ui| {
                for f in &self.ai_agents {
                    ui.selectable_value(&mut choice, f.id.to_string(), f.label).on_hover_text(f.path.display().to_string());
                }
                ui.selectable_value(&mut choice, "custom".to_string(), t("Custom command"));
            });
            if ui.small_button("⟳").on_hover_text(t("Look for installed agents again")).clicked() {
                self.ai_agents = crate::ai::detect();
            }
        });
        if choice != self.settings.ai_agent {
            self.settings.ai_agent = choice.clone();
            self.settings.save();
        }
        if choice == "custom" {
            ui.horizontal(|ui| {
                ui.label(t("Command"));
                if ui.add(egui::TextEdit::singleline(&mut self.settings.ai_command).hint_text("ollama run llama3.1").desired_width(f32::INFINITY)).lost_focus() {
                    self.settings.save();
                }
            });
            ui.weak(t("The request is sent to the command's input; its output is the answer."));
        } else if choice == "ollama" {
            ui.horizontal(|ui| {
                ui.label(t("Model"));
                if ui.add(egui::TextEdit::singleline(&mut self.settings.ai_model).hint_text(t("first installed")).desired_width(f32::INFINITY)).lost_focus() {
                    self.settings.save();
                }
            });
        }
        if self.ai_agents.is_empty() && choice != "custom" {
            ui.colored_label(Color32::from_rgb(200, 90, 40), t("No AI agent was found on this computer. Install Claude Code, Codex CLI, Gemini CLI or Ollama, or set a custom command."));
        }
        ui.separator();
        // the request box sits at the bottom, the conversation fills the rest
        egui::TopBottomPanel::bottom("ai_input").show_separator_line(false).show_inside(ui, |ui| {
            ui.add_space(4.0);
            if let Some(job) = &self.ai_job {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("{} ({} s)", t("Working..."), job.started.elapsed().as_secs()));
                    if ui.button(t("Stop")).clicked() {
                        job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                });
            }
            let r = ui.add(
                egui::TextEdit::multiline(&mut self.ai_input)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text(t("e.g. Draw a class diagram of a library: Book, Member, Loan")),
            );
            let send_key = r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.command);
            ui.horizontal(|ui| {
                let can = self.ai_job.is_none() && !self.ai_input.trim().is_empty();
                if ui.add_enabled(can, egui::Button::new(t("Send"))).clicked() || (send_key && can) {
                    self.ai_send();
                }
                ui.weak(t("Ctrl+Enter sends · Undo takes a change back"));
            });
            ui.add_space(4.0);
        });
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            if self.ai_chat.is_empty() {
                ui.weak(t("Ask the AI agent to draw a new diagram or to change the open one, in your own words. It sees the open diagram."));
            }
            for (user, text) in &self.ai_chat {
                let (who, color) = if *user { (t("You"), ui.visuals().hyperlink_color) } else { ("AI".to_string(), ui.visuals().strong_text_color()) };
                ui.label(egui::RichText::new(who).small().strong().color(color));
                ui.add(egui::Label::new(text.as_str()).selectable(true).wrap());
                ui.add_space(6.0);
            }
        });
    }

    // ---------- undo ----------

    fn checkpoint(&mut self) {
        let Some(id) = self.active else { return };
        if let Some(d) = self.diag().cloned() {
            self.undo.push((id, d));
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.dirty = true;
        }
        self.prop_session = None;
    }

    fn edit_checkpoint(&mut self, key: (u64, u8)) {
        if self.prop_session != Some(key) {
            self.checkpoint();
            self.prop_session = Some(key);
        }
        self.dirty = true;
    }

    fn undo(&mut self) {
        if let Some((id, d)) = self.undo.pop() {
            if let Some(p) = self.project.as_mut() {
                if let Some(cur) = p.diagram_mut(id) {
                    let old = std::mem::replace(cur, d);
                    self.redo.push((id, old));
                }
            }
            self.open_diagram(id);
            self.clear_selection();
            self.dirty = true;
        }
    }

    fn redo(&mut self) {
        if let Some((id, d)) = self.redo.pop() {
            if let Some(p) = self.project.as_mut() {
                if let Some(cur) = p.diagram_mut(id) {
                    let old = std::mem::replace(cur, d);
                    self.undo.push((id, old));
                }
            }
            self.open_diagram(id);
            self.clear_selection();
            self.dirty = true;
        }
    }

    // ---------- editing ----------

    fn create_shape(&mut self, kind: ShapeKind, x: f32, y: f32) {
        let snap = self.settings.snap;
        let g = self.settings.grid_size.max(1.0);
        let Some(active) = self.active else { return };
        self.checkpoint();
        let Some(p) = self.project.as_mut() else { return };
        let id = p.fresh_id();
        let Some(d) = p.diagram_mut(active) else { return };
        let dkind = d.kind;
        let (w, h) = kind.default_size();
        let mut sx = x - w / 2.0;
        let mut sy = y - h / 2.0;
        if kind.is_lifeline() && dkind == DiagramKind::Sequence {
            // lifelines line up along the top
            if let Some(first) = d.shapes.iter().find(|s| s.kind.is_lifeline()) {
                sy = first.y;
            } else {
                sy = y - 20.0;
            }
        }
        if snap {
            sx = (sx / g).round() * g;
            sy = (sy / g).round() * g;
        }
        let base = palette::base_name(kind);
        let name = if base.is_empty() {
            String::new()
        } else {
            let mut n = 1;
            loop {
                let candidate = if n == 1 { base.to_string() } else { format!("{base}{n}") };
                if !d.shapes.iter().any(|s| s.name == candidate) {
                    break candidate;
                }
                n += 1;
            }
        };
        let attributes = match kind {
            ShapeKind::Entity => vec!["PK id : integer".to_string()],
            ShapeKind::DbView => vec!["id : integer".to_string()],
            _ => vec![],
        };
        d.shapes.push(Shape {
            id,
            kind,
            x: sx,
            y: sy,
            w,
            h,
            name: if kind.is_swimlane() { String::new() } else { name.clone() },
            stereotype: String::new(),
            attributes,
            operations: vec![],
            fill: None,
            documentation: String::new(),
            font: None,
            font_size: None,
            orient: 0,
            hide_icon: false,
            hide_attributes: false,
            hide_operations: false,
            splits: Vec::new(),
            act_spans: Vec::new(),
            dividers: None,
            partitions: if kind.is_swimlane() {
                let half = if kind == ShapeKind::VerticalSwimlane { w / 2.0 } else { h / 2.0 };
                Some(vec![crate::model::Partition { name: "Partition".into(), size: half }, crate::model::Partition { name: "Partition2".into(), size: half }])
            } else {
                None
            },
            label_offset: (0.0, 0.0),
        });
        self.clear_selection();
        self.sel_shapes.push(id);
        if kind == ShapeKind::Port {
            self.snap_port_to_owner(id);
        }
        let editable = (!kind.is_pseudo() || kind == ShapeKind::Actor || kind.is_network_device()) && !matches!(kind, ShapeKind::AltFragment | ShapeKind::LoopFragment) && !kind.is_swimlane();
        if editable {
            self.editing = Some(InlineEdit { target: EditTarget::Shape(id), text: name, focus: true, line: None, new_line: false, part: None });
        }
        if !self.sticky {
            self.tool = Tool::Select;
        }
    }

    /// A provided interface (ball) or required interface (socket) among the moved shapes that
    /// was dropped overlapping one of the other kind snaps onto it, centre on centre, so the
    /// ball sits in the socket; from then on they move together.
    fn join_interfaces(&mut self, moved: &[u64]) {
        let Some(d) = self.diag_mut() else { return };
        for &id in moved {
            let Some(s) = d.shape(id).cloned() else { continue };
            if !is_interface_end(s.kind) || !joined_interfaces(d, &s).is_empty() {
                continue;
            }
            let (cx, cy) = s.center();
            let reach = |o: &Shape| (s.w.max(s.h) + o.w.max(o.h)) / 2.0;
            let target = d
                .shapes
                .iter()
                .filter(|o| !moved.contains(&o.id) && opposite_interface(s.kind, o.kind))
                .map(|o| {
                    let (ox, oy) = o.center();
                    (o.center(), ((ox - cx).powi(2) + (oy - cy).powi(2)).sqrt(), reach(o))
                })
                .filter(|(_, dist, r)| dist <= r)
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(((ox, oy), _, _)) = target {
                if let Some(m) = d.shape_mut(id) {
                    m.x = ox - m.w / 2.0;
                    m.y = oy - m.h / 2.0;
                }
                // the socket's cup opens toward the side the ball's line comes from (away from the
                // shape that requires the interface)
                let pair_ids: Vec<u64> = d.shapes.iter().filter(|o| {
                    let (px, py) = o.center();
                    is_interface_end(o.kind) && (px - ox).abs() <= 1.5 && (py - oy).abs() <= 1.5
                }).map(|o| o.id).collect();
                let other_end = |sid: u64| -> Option<(f32, f32)> {
                    d.connectors.iter().find_map(|c| {
                        let o = if c.from == sid { c.to } else if c.to == sid { c.from } else { return None };
                        d.shape(o).filter(|x| !pair_ids.contains(&x.id)).map(|x| x.center())
                    })
                };
                let sock = pair_ids.iter().copied().find(|i| d.shape(*i).map(|x| x.kind == ShapeKind::RequiredInterface).unwrap_or(false));
                let ball = pair_ids.iter().copied().find(|i| d.shape(*i).map(|x| x.kind == ShapeKind::ProvidedInterface).unwrap_or(false));
                // direction the cup should open: toward the provider, or away from the requirer
                let dir = ball.and_then(other_end).map(|(px, py)| (px - ox, py - oy)).or_else(|| sock.and_then(other_end).map(|(qx, qy)| (ox - qx, oy - qy)));
                if let (Some(sid), Some((dx, dy))) = (sock, dir) {
                    let orient = if dx.abs() >= dy.abs() { if dx >= 0.0 { 0 } else { 2 } } else if dy >= 0.0 { 1 } else { 3 };
                    if let Some(m) = d.shape_mut(sid) {
                        m.orient = orient;
                    }
                }
                // both names would sit under the pair on top of each other: the socket's goes above
                let pair: Vec<Shape> = d.shapes.iter().filter(|o| o.id == id || joined_interfaces(d, &s).contains(&o.id) || {
                    let (px, py) = o.center();
                    o.id != id && opposite_interface(s.kind, o.kind) && (px - ox).abs() <= 1.5 && (py - oy).abs() <= 1.5
                }).cloned().collect();
                if pair.len() >= 2 && pair.iter().all(|p| !p.name.is_empty()) {
                    if let Some(sock) = pair.iter().find(|p| p.kind == ShapeKind::RequiredInterface) {
                        if sock.label_offset == (0.0, 0.0) {
                            let lift = -(sock.h + 20.0);
                            if let Some(m) = d.shape_mut(sock.id) {
                                m.label_offset = (0.0, lift);
                            }
                        }
                    }
                }
            }
        }
    }

    /// If the shape is a port near a component (or node, class…), put it on that shape's edge.
    fn snap_port_to_owner(&mut self, id: u64) {
        let Some(d) = self.diag_mut() else { return };
        let Some(port) = d.shape(id).cloned() else { return };
        if port.kind != ShapeKind::Port {
            return;
        }
        let Some(owner) = port_owner(d, &port).and_then(|o| d.shape(o).cloned()) else { return };
        if let Some(p) = d.shape_mut(id) {
            snap_port(p, &owner);
        }
    }

    fn create_connector(&mut self, kind: ConnKind, from: u64, to: u64, y: f32) {
        let Some(d) = self.diag() else { return };
        let (Some(a), Some(b)) = (d.shape(from), d.shape(to)) else { return };
        if from == to && !kind.allows_self() {
            return;
        }
        if kind.is_message() && d.kind == DiagramKind::Sequence && !(a.kind.is_lifeline() && b.kind.is_lifeline()) {
            self.status = "Messages connect two lifelines".into();
            return;
        }
        if kind.is_erd() && !(matches!(a.kind, ShapeKind::Entity | ShapeKind::DbView) && matches!(b.kind, ShapeKind::Entity | ShapeKind::DbView)) {
            self.status = "Relationships connect two entities".into();
            return;
        }
        // a line to an interface ball is a Realization (vpplatform.jar v.dwx.b.a);
        // a line to a socket is a Usage (estimated). The interface is kept as the target.
        let iface = |k: ShapeKind| matches!(k, ShapeKind::ProvidedInterface | ShapeKind::RequiredInterface);
        let plain = matches!(kind, ConnKind::Association | ConnKind::Link | ConnKind::Wireless | ConnKind::Dependency | ConnKind::Realization | ConnKind::Usage);
        let (from, to, a, b) = if plain && iface(a.kind) && !iface(b.kind) { (to, from, b, a) } else { (from, to, a, b) };
        let kind = if plain && b.kind == ShapeKind::ProvidedInterface {
            ConnKind::Realization
        } else if plain && b.kind == ShapeKind::RequiredInterface {
            ConnKind::Usage
        } else {
            kind
        };
        let head = |s: &Shape| if s.kind == ShapeKind::ActorLifeline { 85.0 } else { 55.0 };
        // a Create message meets the head of the lifeline it creates, so only its sender limits it
        let created = kind == ConnKind::CreateMessage && from != to;
        let min_y = if created { a.y + head(a) } else { (a.y + head(a)).max(b.y + head(b)) };
        let max_y = if created { a.y + a.h - 10.0 } else { (a.y + a.h).min(b.y + b.h) - 10.0 };
        let y = y.clamp(min_y, max_y.max(min_y));
        self.checkpoint();
        let p = self.project.as_mut().unwrap();
        let id = p.fresh_id();
        let d = p.diagram_mut(self.active.unwrap()).unwrap();
        // new lines start without a name (messages too: no "message()" placeholder)
        let name = String::new();
        d.connectors.push(Connector {
            id,
            kind,
            from,
            to,
            name,
            from_label: String::new(),
            to_label: String::new(),
            y,
            documentation: String::new(),
            points: vec![],
            arrow_start: false,
            arrow_end: false,
            style: render::STYLE_FOLLOW_DIAGRAM,
            nav_from: crate::model::NAV_TRUE,
            nav_to: crate::model::NAV_TRUE,
            seq_no: String::new(),
            from_label_offset: (0.0, 0.0),
            to_label_offset: (0.0, 0.0),
            label_offset: (0.0, 0.0),
            from_anchor: None,
            to_anchor: None,
        });
        self.clear_selection();
        self.sel_conns.push(id);
        if !self.sticky {
            self.tool = Tool::Select;
        }
    }

    /// A message from a lifeline dropped where there is no lifeline: make one there (centred on
    /// the drop point) and connect them, in one undo step. A Create message's lifeline has its
    /// head at the message; for other messages it joins the lifelines along the top.
    fn message_to_new_lifeline(&mut self, kind: ConnKind, from: u64, x: f32, press_y: f32) {
        let Some(d) = self.diag() else { return };
        let Some(src) = d.shape(from).cloned() else { return };
        let head = |s: &Shape| if s.kind == ShapeKind::ActorLifeline { 85.0 } else { 55.0 };
        let msg_y = press_y.clamp(src.y + head(&src), (src.y + src.h - 10.0).max(src.y + head(&src)));
        let top = d.shapes.iter().filter(|s| y_locked(d, s)).map(|s| s.y).fold(f32::INFINITY, f32::min);
        let top = if top.is_finite() { top } else { src.y };
        let bottom = d.shapes.iter().filter(|s| s.kind.is_lifeline()).map(|s| s.y + s.h).fold(src.y + src.h, f32::max);
        let y = if kind == ConnKind::CreateMessage { msg_y - lifeline_head(ShapeKind::Lifeline) / 2.0 } else { top };
        let h = (bottom - y).max(msg_y - y + 60.0);
        let undo_before = self.undo.len();
        self.create_shape(ShapeKind::Lifeline, x, y);
        let Some(&new_id) = self.sel_shapes.first() else { return };
        if let Some(dm) = self.diag_mut() {
            if let Some(s) = dm.shape_mut(new_id) {
                s.x = x - s.w / 2.0;
                s.y = y;
                s.h = h;
            }
        }
        let before = self.diag().map(|dm| dm.connectors.len()).unwrap_or(0);
        self.create_connector(kind, from, new_id, msg_y);
        if self.diag().map(|dm| dm.connectors.len()).unwrap_or(0) > before && self.undo.len() > undo_before + 1 {
            // the shape and its message undo together
            self.undo.pop();
        }
        // the new lifeline stays selected, with its name being typed
        self.clear_selection();
        self.sel_shapes.push(new_id);
    }

    /// Add a section at the bottom of an alternative fragment (splitting the last section; the
    /// fragment grows if that section is too short). It becomes the [else] section; a section
    /// that was [else] before keeps an empty guard to fill in.
    fn add_alt_section(&mut self, id: u64) {
        self.checkpoint();
        let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) else { return };
        let mut v = s.alt_dividers();
        let top = v.last().map(|d| d.y).unwrap_or(crate::model::ALT_HEAD);
        if s.h - top < 60.0 {
            s.h = top + 80.0;
        }
        let y = (top + s.h) / 2.0;
        let guard = if v.iter().any(|d| d.guard.trim() == "else") {
            if let Some(last) = v.last_mut().filter(|d| d.guard.trim() == "else") {
                last.guard.clear();
                "else".to_string()
            } else {
                String::new()
            }
        } else {
            "else".to_string()
        };
        v.push(crate::model::Divider { y, guard });
        s.dividers = Some(v);
    }

    /// Take a section of an alternative fragment away: the one at `at` (height below the
    /// fragment's top) unless that is the first, else the last one. The section above takes its
    /// room.
    fn remove_alt_section(&mut self, id: u64, at: Option<f32>) {
        let count = self.diag().and_then(|d| d.shape(id)).map(|s| s.alt_dividers().len()).unwrap_or(0);
        if count == 0 {
            return;
        }
        self.checkpoint();
        let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) else { return };
        let mut v = s.alt_dividers();
        let idx = at.and_then(|y| v.iter().rposition(|d| d.y <= y)).unwrap_or(v.len() - 1);
        v.remove(idx);
        s.dividers = Some(v);
    }

    /// Insert a partition into a swimlane after partition `after`; the swimlane grows and what
    /// stood in the partitions after it moves along with them.
    fn add_partition(&mut self, id: u64, after: usize) {
        self.checkpoint();
        let Some(dm) = self.diag_mut() else { return };
        let Some(before) = dm.shape(id).cloned() else { return };
        let Some((at, size)) = dm.shape_mut(id).map(|s| s.add_partition(after)) else { return };
        shift_lane_contents(dm, &before, at, size);
    }

    /// Take a partition out of a swimlane; the swimlane shrinks and what stood in the
    /// partitions after it moves back. What stood in the removed partition stays where it is.
    fn remove_partition(&mut self, id: u64, idx: usize) {
        if self.diag().and_then(|d| d.shape(id)).map(|s| s.lane_partitions().len() < 2).unwrap_or(true) {
            return;
        }
        self.checkpoint();
        let Some(dm) = self.diag_mut() else { return };
        let Some(before) = dm.shape(id).cloned() else { return };
        if let Some(Some((at, size))) = dm.shape_mut(id).map(|s| s.remove_partition(idx)) {
            shift_lane_contents(dm, &before, at + size, -size);
        }
    }

    /// The selected activation bar on screen, with its top and bottom handle points.
    fn bar_handles(&self, d: &Diagram, cam: &Cam) -> Option<(Rect, Pos2, Pos2)> {
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
    fn bar_click(&mut self, d: &Diagram, cam: &Cam, p: Pos2) -> bool {
        if d.kind != DiagramKind::Sequence || matches!(self.hit_any(cam, p), Some(EditTarget::Conn(_))) || self.label_at(p).is_some() {
            return false;
        }
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let Some(b) = bar_at(d, &opts, cam.zoom, cam.to_diagram(p)) else { return false };
        self.clear_selection();
        self.sel_bar = Some((b.lifeline, b.depth, (b.y0 + b.y1) / 2.0));
        true
    }

    /// End the lifeline's activation at this height (for a call whose return message is left
    /// out); its next message starts a new activation.
    fn split_activation(&mut self, lifeline: u64, y: f32) {
        self.checkpoint();
        if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(lifeline)) {
            s.splits.push(y);
            s.splits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    /// Take a split away again, so the activations on either side join.
    fn join_activations(&mut self, lifeline: u64, y: f32) {
        self.checkpoint();
        if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(lifeline)) {
            s.splits.retain(|v| (v - y).abs() > 0.01);
        }
    }

    /// Attach a note: between two shapes, or from a shape (the note) to a line.
    fn create_anchor(&mut self, a: u64, b: u64) {
        let Some(d) = self.diag() else { return };
        let (sa, sb) = (d.shape(a).is_some(), d.shape(b).is_some());
        if sa && sb {
            self.create_connector(ConnKind::Anchor, a, b, 0.0);
            return;
        }
        let (from, to) = if sa { (a, b) } else if sb { (b, a) } else { return };
        if !d.connectors.iter().any(|c| c.id == to) {
            return;
        }
        self.checkpoint();
        let p = self.project.as_mut().unwrap();
        let id = p.fresh_id();
        let Some(d) = p.diagram_mut(self.active.unwrap()) else { return };
        d.connectors.push(Connector {
            id,
            kind: ConnKind::Anchor,
            from,
            to,
            name: String::new(),
            from_label: String::new(),
            to_label: String::new(),
            y: 0.0,
            documentation: String::new(),
            points: vec![],
            arrow_start: false,
            arrow_end: false,
            style: render::STYLE_FOLLOW_DIAGRAM,
            nav_from: crate::model::NAV_TRUE,
            nav_to: crate::model::NAV_TRUE,
            seq_no: String::new(),
            from_label_offset: (0.0, 0.0),
            to_label_offset: (0.0, 0.0),
            label_offset: (0.0, 0.0),
            from_anchor: None,
            to_anchor: None,
        });
        self.clear_selection();
        self.sel_conns.push(id);
        if !self.sticky {
            self.tool = Tool::Select;
        }
    }

    fn delete_selection(&mut self) {
        if self.sel_shapes.is_empty() && self.sel_conns.is_empty() {
            return;
        }
        self.checkpoint();
        let shapes = self.sel_shapes.clone();
        let conns = self.sel_conns.clone();
        if let Some(d) = self.diag_mut() {
            d.shapes.retain(|s| !shapes.contains(&s.id));
            d.connectors.retain(|c| !conns.contains(&c.id) && !shapes.contains(&c.from) && !shapes.contains(&c.to));
            // notes attached to a removed line go with it
            let live: std::collections::HashSet<u64> = d.connectors.iter().map(|c| c.id).chain(d.shapes.iter().map(|s| s.id)).collect();
            d.connectors.retain(|c| live.contains(&c.to));
        }
        self.clear_selection();
    }

    fn copy_selection(&mut self) -> bool {
        let Some(d) = self.diag() else { return false };
        let shapes: Vec<Shape> = d.shapes.iter().filter(|s| self.sel_shapes.contains(&s.id)).cloned().collect();
        if shapes.is_empty() {
            return false;
        }
        let ids: Vec<u64> = shapes.iter().map(|s| s.id).collect();
        let conns: Vec<Connector> = d.connectors.iter().filter(|c| ids.contains(&c.from) && ids.contains(&c.to)).cloned().collect();
        self.clipboard = Some((shapes, conns));
        true
    }

    fn paste_items(&mut self, shapes: Vec<Shape>, conns: Vec<Connector>, offset: f32) {
        let Some(active) = self.active else { return };
        if shapes.is_empty() {
            return;
        }
        self.checkpoint();
        let p = self.project.as_mut().unwrap();
        let mut map = HashMap::new();
        let mut new_shapes = vec![];
        for mut s in shapes {
            let n = p.fresh_id();
            map.insert(s.id, n);
            s.id = n;
            s.x += offset;
            s.y += offset;
            new_shapes.push(s);
        }
        let mut new_conns = vec![];
        for mut c in conns {
            c.id = p.fresh_id();
            c.from = map[&c.from];
            c.to = map[&c.to];
            c.y += offset;
            for p in c.points.iter_mut() {
                p.0 += offset;
                p.1 += offset;
            }
            new_conns.push(c);
        }
        self.sel_shapes = new_shapes.iter().map(|s| s.id).collect();
        self.sel_conns = new_conns.iter().map(|c| c.id).collect();
        if let Some(d) = p.diagram_mut(active) {
            d.shapes.extend(new_shapes);
            d.connectors.extend(new_conns);
        }
    }

    fn paste(&mut self) {
        if let Some((s, c)) = self.clipboard.clone() {
            self.paste_items(s, c, 20.0);
            // the next paste lands a little further down
            if let Some((s, c)) = self.clipboard.as_mut() {
                for x in s.iter_mut() {
                    x.x += 20.0;
                    x.y += 20.0;
                }
                for x in c.iter_mut() {
                    x.y += 20.0;
                    for p in x.points.iter_mut() {
                        p.0 += 20.0;
                        p.1 += 20.0;
                    }
                }
            }
        }
    }

    fn cut(&mut self) {
        if self.copy_selection() {
            self.delete_selection();
        }
    }

    fn duplicate(&mut self) {
        let saved = self.clipboard.clone();
        if self.copy_selection() {
            let (s, c) = self.clipboard.clone().unwrap();
            self.paste_items(s, c, 20.0);
        }
        self.clipboard = saved;
    }

    fn select_all(&mut self) {
        if let Some(d) = self.diag() {
            let shapes = d.shapes.iter().map(|s| s.id).collect();
            let conns = d.connectors.iter().map(|c| c.id).collect();
            self.sel_shapes = shapes;
            self.sel_conns = conns;
        }
    }

    fn select_same_type(&mut self) {
        let Some(d) = self.diag() else { return };
        let kinds: Vec<ShapeKind> = d.shapes.iter().filter(|s| self.sel_shapes.contains(&s.id)).map(|s| s.kind).collect();
        let ckinds: Vec<ConnKind> = d.connectors.iter().filter(|c| self.sel_conns.contains(&c.id)).map(|c| c.kind).collect();
        let shapes = d.shapes.iter().filter(|s| kinds.contains(&s.kind)).map(|s| s.id).collect();
        let conns = d.connectors.iter().filter(|c| ckinds.contains(&c.kind)).map(|c| c.id).collect();
        self.sel_shapes = shapes;
        self.sel_conns = conns;
    }

    fn select_inverse(&mut self) {
        let Some(d) = self.diag() else { return };
        let shapes = d.shapes.iter().filter(|s| !self.sel_shapes.contains(&s.id)).map(|s| s.id).collect();
        let conns = d.connectors.iter().filter(|c| !self.sel_conns.contains(&c.id)).map(|c| c.id).collect();
        self.sel_shapes = shapes;
        self.sel_conns = conns;
    }

    /// 0 left, 1 right, 2 top, 3 bottom, 4 horizontal centers, 5 vertical centers
    fn align(&mut self, how: u8) {
        if self.sel_shapes.len() < 2 {
            return;
        }
        self.checkpoint();
        let ids = self.sel_shapes.clone();
        let Some(d) = self.diag_mut() else { return };
        let Some(first) = d.shape(ids[0]).cloned() else { return };
        for s in d.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
            match how {
                0 => s.x = first.x,
                1 => s.x = first.x + first.w - s.w,
                2 => s.y = first.y,
                3 => s.y = first.y + first.h - s.h,
                4 => s.x = first.x + first.w / 2.0 - s.w / 2.0,
                _ => s.y = first.y + first.h / 2.0 - s.h / 2.0,
            }
        }
    }

    /// 0 width, 1 height, 2 both
    fn same_size(&mut self, how: u8) {
        if self.sel_shapes.len() < 2 {
            return;
        }
        self.checkpoint();
        let ids = self.sel_shapes.clone();
        let Some(d) = self.diag_mut() else { return };
        let Some(first) = d.shape(ids[0]).cloned() else { return };
        for s in d.shapes.iter_mut().filter(|s| ids.contains(&s.id) && !s.kind.fixed_size()) {
            if how != 1 {
                s.w = first.w;
            }
            if how != 0 {
                s.h = first.h;
            }
        }
    }

    fn reorder(&mut self, front: bool) {
        if self.sel_shapes.is_empty() {
            return;
        }
        self.checkpoint();
        let ids = self.sel_shapes.clone();
        if let Some(d) = self.diag_mut() {
            let (sel, rest): (Vec<Shape>, Vec<Shape>) = d.shapes.drain(..).partition(|s| ids.contains(&s.id));
            if front {
                d.shapes = rest.into_iter().chain(sel).collect();
            } else {
                d.shapes = sel.into_iter().chain(rest).collect();
            }
        }
    }

    fn add_line(&mut self, operations: bool) {
        let Some(&id) = self.sel_shapes.first() else { return };
        let vis = self.project.as_ref().map(|p| p.options.default_visibility).unwrap_or('-');
        self.checkpoint();
        if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) {
            if operations {
                s.operations.push("+operation() : void".into());
            } else {
                let line = match s.kind {
                    ShapeKind::Entity | ShapeKind::DbView => "column : varchar(255)".to_string(),
                    ShapeKind::Enumeration => format!("LITERAL{}", s.attributes.len() + 1),
                    ShapeKind::Instance => "slot = value".to_string(),
                    ShapeKind::State => "do / activity".to_string(),
                    _ => format!("{vis}attribute : String"),
                };
                s.attributes.push(line);
            }
        }
    }

    /// Zoom In as the original does it (v.ddp.j.bD): +10 points while below 800 %.
    fn zoom_in_step(&mut self, around: Option<Pos2>) {
        let z = self.view().zoom;
        if z < ZOOM_MAX {
            self.zoom_set(z + 0.1, around);
        }
    }

    /// Zoom Out as the original does it (v.ddp.j.bE): -10 points above 10 %,
    /// otherwise x0.9, and never at or below 2 %.
    fn zoom_out_step(&mut self, around: Option<Pos2>) {
        let z = self.view().zoom;
        let n = if z > 0.1 { z - 0.1 } else { z * 0.9 };
        if n > ZOOM_MIN {
            self.zoom_set(n, around);
        }
    }

    fn zoom_set(&mut self, z: f32, around: Option<Pos2>) {
        let f = z / self.view().zoom;
        self.zoom_by(f, around);
    }

    fn zoom_by(&mut self, factor: f32, around: Option<Pos2>) {
        let rect = self.canvas_rect;
        let Some(v) = self.view_mut() else { return };
        let old = v.zoom;
        let new = (old * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let anchor = around.unwrap_or(if rect.is_positive() { rect.center() } else { Pos2::ZERO });
        // keep the diagram point under the anchor fixed
        let rel = anchor - rect.min - v.pan;
        v.pan += rel - rel * (new / old);
        v.zoom = new;
    }

    fn zoom_to(&mut self, z: f32) {
        let f = z / self.view().zoom;
        self.zoom_by(f, None);
    }

    fn fit_to_window(&mut self) {
        self.fit_view(2.0, false);
    }

    /// Fit the diagram into the canvas, never zooming in past `max_zoom`.
    /// With `only_if_needed`, a diagram that already fits at 100% is left alone.
    fn fit_view(&mut self, max_zoom: f32, only_if_needed: bool) {
        let rect = self.canvas_rect;
        let Some(d) = self.diag() else { return };
        if d.shapes.is_empty() || !rect.is_positive() {
            if let Some(v) = self.view_mut() {
                v.zoom = 1.0;
                v.pan = Vec2::new(20.0, 20.0);
            }
            return;
        }
        let mut min = Pos2::new(f32::MAX, f32::MAX);
        let mut max = Pos2::new(f32::MIN, f32::MIN);
        for s in &d.shapes {
            min.x = min.x.min(s.x);
            min.y = min.y.min(s.y - 30.0);
            max.x = max.x.max(s.x + s.w + 30.0);
            max.y = max.y.max(s.y + s.h + 20.0);
        }
        let w = (max.x - min.x).max(1.0);
        let h = (max.y - min.y).max(1.0);
        let z = ((rect.width() - 40.0) / w).min((rect.height() - 40.0) / h).clamp(0.1, max_zoom);
        if only_if_needed && 20.0 + max.x <= rect.width() && 20.0 + max.y <= rect.height() && min.x >= -20.0 && min.y >= -20.0 {
            return;
        }
        if let Some(v) = self.view_mut() {
            v.zoom = z;
            v.pan = if only_if_needed {
                // first look: keep the diagram at the top left, like at 100%
                Vec2::new(20.0 - min.x * z, 20.0 - (min.y + 30.0) * z)
            } else {
                Vec2::new(rect.width() / 2.0 - (min.x + w / 2.0) * z, rect.height() / 2.0 - (min.y + h / 2.0) * z)
            };
        }
    }

    // ---------- printing ----------

    fn open_print_dialog(&mut self) {
        let Some(p) = self.project.as_ref() else { return };
        if self.active.is_none() {
            return;
        }
        // the open diagram is ticked; others can be added (each prints on its own page)
        let chosen = p.diagrams.iter().map(|d| (d.id, Some(d.id) == self.active)).collect();
        self.print_dlg = Some(PrintDlg { paper: crate::print::Paper::from_id(&self.settings.print_paper), orient: crate::print::Orient::from_code(self.settings.print_orient), chosen, pk: Picker::default() });
    }

    fn print_dialog(&mut self, ctx: &egui::Context) {
        use crate::print::{Orient, Paper};
        let Some(mut dlg) = self.print_dlg.take() else { return };
        let Some(project) = self.project.clone() else { return };
        let lang = self.settings.language;
        let t = |s: &str| tr(lang, s).to_string();
        let tabs = self.tabs.clone();
        let current = self.active;
        let esc = esc_cancels(ctx);
        let mut action: Option<PrintTo> = None;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new(t("Print"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                modal_dialog(ui);
                ui.set_width(420.0);
                egui::Grid::new("print_opts").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label(t("Paper size"));
                    egui::ComboBox::from_id_salt("print_paper").width(220.0).selected_text(dlg.paper.label()).show_ui(ui, |ui| {
                        for p in Paper::ALL {
                            ui.selectable_value(&mut dlg.paper, p, p.label());
                        }
                    });
                    ui.end_row();
                    ui.label(t("Orientation"));
                    egui::ComboBox::from_id_salt("print_orient").width(220.0).selected_text(t(dlg.orient.label())).show_ui(ui, |ui| {
                        for o in Orient::ALL {
                            ui.selectable_value(&mut dlg.orient, o, t(o.label()));
                        }
                    });
                    ui.end_row();
                });
                ui.weak(t("Each diagram is printed on its own page, made as large as the paper allows (10 mm margins)."));
                ui.add_space(6.0);
                ui.label(t("Diagrams to print:"));
                diagram_picker(ui, "print_pick", lang, &project, &mut dlg.chosen, &mut dlg.pk, &tabs, current, 300.0);
                ui.separator();
                let n = dlg.chosen.iter().filter(|c| c.1).count();
                ui.horizontal(|ui| {
                    if ui.add_enabled(n > 0, egui::Button::new(format!("{} ({n})", t("Print...")))).clicked() {
                        action = Some(PrintTo::Printer);
                    }
                    if ui.add_enabled(n > 0, egui::Button::new(t("Save as PDF..."))).clicked() {
                        let name = if n == 1 {
                            dlg.chosen.iter().find(|c| c.1).and_then(|c| project.diagram(c.0)).map(|d| file_name_for(&d.name)).unwrap_or_default()
                        } else {
                            file_name_for(&project.name)
                        };
                        if let Some(mut p) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{name}.pdf")).save_file() {
                            if p.extension().is_none() {
                                p.set_extension("pdf");
                            }
                            action = Some(PrintTo::Pdf(p));
                        }
                    }
                    if ui.button(t("Cancel")).clicked() {
                        cancel = true;
                    }
                });
            });
        if let Some(to) = action {
            self.settings.print_paper = dlg.paper.id().to_string();
            self.settings.print_orient = dlg.orient.code();
            self.settings.save();
            let ids: Vec<u64> = dlg.chosen.iter().filter(|c| c.1).map(|c| c.0).collect();
            self.start_print(ids, to, dlg.paper, dlg.orient);
        } else if !(cancel || esc || !open) {
            self.print_dlg = Some(dlg);
        }
    }

    fn start_print(&mut self, ids: Vec<u64>, to: PrintTo, paper: crate::print::Paper, orient: crate::print::Orient) {
        if ids.is_empty() {
            return;
        }
        self.commit_edit();
        self.menu = None;
        let total = ids.len();
        self.printing = Some(PrintJob { to, paper, orient, queue: ids.into_iter().collect(), total, pages: vec![], wait: 3, finishing: false });
        self.job_cancel = false;
        self.status = format!("{} 1/{total}", tr(self.settings.language, "Preparing pages"));
    }

    /// The picture of one diagram drawn off screen, at `scale` pixels per unit and at most
    /// `max_px` on its longer side.
    fn picture_of(&self, id: u64, scale: f32, max_px: f32) -> Option<image::RgbImage> {
        let p = self.project.as_ref()?;
        let d = p.diagram(id)?;
        Some(crate::offscreen::diagram_picture(d, &p.options, &self.look(), &self.font_defs, scale, max_px))
    }

    /// Printing and exporting several diagrams: one diagram each frame, so the window keeps
    /// answering and shows how far it got. Esc stops.
    fn work_jobs(&mut self, ctx: &egui::Context) {
        let lang = self.settings.language;
        let esc = ctx.input(|i| i.key_pressed(Key::Escape)) || std::mem::take(&mut self.job_cancel);
        if self.printing.is_some() {
            if esc {
                self.printing = None;
                self.status = tr(lang, "Printing cancelled.").to_string();
                return;
            }
            ctx.request_repaint();
            // let the progress window show first (and say what comes next)
            if let Some(j) = self.printing.as_mut() {
                if j.wait > 0 {
                    j.wait -= 1;
                    return;
                }
                if j.finishing {
                    self.finish_print();
                    return;
                }
            }
            let next = self.printing.as_mut().and_then(|j| j.queue.pop_front());
            let paper = self.printing.as_ref().map(|j| j.paper).unwrap_or(crate::print::Paper::A4);
            if let Some(id) = next {
                // sharp on paper: about 250 dots per inch over the printable length
                if let Some(pic) = self.picture_of(id, 8.0, crate::print::target_long_px(paper)) {
                    if let Some(j) = self.printing.as_mut() {
                        j.pages.push(pic);
                    }
                }
            }
            if let Some(j) = self.printing.as_mut() {
                if j.queue.is_empty() {
                    j.finishing = true;
                    j.wait = 2;
                } else {
                    self.status = format!("{} {}/{}", tr(lang, "Preparing pages"), j.pages.len() + 1, j.total);
                }
            }
            return;
        }
        if self.batch.is_some() {
            ctx.request_repaint();
            if esc {
                if let Some(b) = self.batch.as_mut() {
                    b.queue.clear();
                }
            }
            if let Some(b) = self.batch.as_mut() {
                if b.wait > 0 && !b.queue.is_empty() {
                    b.wait -= 1;
                    return;
                }
            }
            let next = self.batch.as_mut().and_then(|b| b.queue.pop_front());
            if let Some((id, path)) = next {
                let ok = self.picture_of(id, 2.0, 8000.0).map(|pic| save_picture(&pic, &path)).unwrap_or(false);
                if let Some(b) = self.batch.as_mut() {
                    if ok {
                        b.done += 1;
                    } else {
                        b.failed += 1;
                    }
                }
            }
            if let Some(b) = self.batch.as_ref() {
                if b.queue.is_empty() {
                    let b = self.batch.take().unwrap();
                    self.status = if b.failed == 0 {
                        format!("{} {} → {}", b.done, tr(lang, "images exported"), b.folder.display())
                    } else {
                        format!("{} {}, {} {} → {}", b.done, tr(lang, "images exported"), b.failed, tr(lang, "failed"), b.folder.display())
                    };
                } else {
                    self.status = format!("{} {}/{}...", tr(lang, "Exporting"), b.done + b.failed + 1, b.total);
                }
            }
            ctx.request_repaint();
        }
    }

    /// A progress window over everything while pages are prepared or images exported, with a
    /// progress bar, what is being done and a Cancel button (Esc works too).
    fn job_progress(&mut self, ctx: &egui::Context) {
        let lang = self.settings.language;
        let t = |s: &str| tr(lang, s).to_string();
        let (title, line, done, total) = if let Some(j) = &self.printing {
            let to_pdf = matches!(j.to, PrintTo::Pdf(_));
            let title = t(if to_pdf { "Saving as PDF..." } else { "Preparing to print..." });
            let line = if j.finishing {
                t(if to_pdf { "Writing the PDF file..." } else { "Opening the Print dialog..." })
            } else {
                format!("{} {} / {}", t("Preparing page"), (j.pages.len() + 1).min(j.total), j.total)
            };
            (title, line, j.pages.len(), j.total)
        } else if let Some(b) = &self.batch {
            let n = b.done + b.failed;
            (t("Exporting images..."), format!("{} {} / {}", t("Exporting image"), (n + 1).min(b.total), b.total), n, b.total)
        } else {
            return;
        };
        ctx.set_cursor_icon(egui::CursorIcon::Progress);
        let mut cancel = false;
        // no fade-in: frames are slow while pages are drawn, so a fade would leave the window
        // faint for the first few pages
        let id = egui::Id::new("job_progress");
        egui::Modal::new(id).area(egui::Modal::default_area(id).fade_in(false)).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.heading(title);
            });
            ui.add_space(6.0);
            ui.label(line);
            ui.add(egui::ProgressBar::new(if total == 0 { 0.0 } else { done as f32 / total as f32 }).show_percentage());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(t("Cancel")).clicked() {
                    cancel = true;
                }
                ui.weak(t("Esc also cancels."));
            });
        });
        if cancel {
            self.job_cancel = true;
        }
    }

    /// Every page is ready: print it or write the PDF.
    fn finish_print(&mut self) {
        let Some(job) = self.printing.take() else { return };
        let lang = self.settings.language;
        let n = job.pages.len();
        let doc = self.project.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "NapkinUML".into());
        if let (Ok(pdf), true) = (std::env::var("NAPKINUML_PRINT_PDF"), self.screenshot.is_some()) {
            for (i, page) in job.pages.iter().enumerate() {
                let _ = page.save(format!("{pdf}.page{}.png", i + 1));
            }
        }
        self.status = match job.to {
            PrintTo::Pdf(path) => match crate::print::write_pdf(&path, &job.pages, job.paper, job.orient) {
                Ok(()) => format!("{n} {} → {}", tr(lang, "pages saved as PDF"), path.display()),
                Err(e) => format!("{}: {e}", tr(lang, "Could not save the PDF")),
            },
            PrintTo::Printer => {
                #[cfg(windows)]
                {
                    match crate::print::print_windows(&job.pages, job.paper, job.orient, &doc) {
                        Ok(true) => format!("{n} {}", tr(lang, "pages sent to the printer")),
                        Ok(false) => tr(lang, "Printing cancelled.").to_string(),
                        Err(e) => format!("{}: {e}", tr(lang, "Could not print")),
                    }
                }
                #[cfg(not(windows))]
                {
                    // elsewhere the pages open as a PDF in the system viewer, to print from there
                    let path = std::env::temp_dir().join(format!("{}.pdf", file_name_for(&doc)));
                    match crate::print::write_pdf(&path, &job.pages, job.paper, job.orient).and_then(|_| crate::print::open_with_system(&path)) {
                        Ok(()) => tr(lang, "The pages opened in the PDF viewer; print them from there.").to_string(),
                        Err(e) => format!("{}: {e}", tr(lang, "Could not print")),
                    }
                }
            }
        };
    }

    fn open_export_dialog(&mut self) {
        let Some(p) = self.project.as_ref() else { return };
        // every diagram is ticked at first; the folder starts beside the project file
        let chosen = p.diagrams.iter().map(|d| (d.id, true)).collect();
        let folder = self.path.as_ref().and_then(|f| f.parent().map(|d| d.to_path_buf()));
        let format = ImgFormat::from_id(&self.settings.export_format);
        self.export_dlg = Some(ExportDlg { chosen, folder, pk: Picker::default(), format });
    }

    fn export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.export_dlg.take() else { return };
        let Some(project) = self.project.clone() else { return };
        let lang = self.settings.language;
        let t = |s: &str| tr(lang, s).to_string();
        let tabs = self.tabs.clone();
        let current = self.active;
        let esc = esc_cancels(ctx);
        let mut go = false;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new(t("Export Images"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                modal_dialog(ui);
                ui.set_width(420.0);
                ui.label(t("Diagrams to export (one image each):"));
                diagram_picker(ui, "export_pick", lang, &project, &mut dlg.chosen, &mut dlg.pk, &tabs, current, 300.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(t("Folder"));
                    let shown = dlg.folder.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| t("(not chosen)"));
                    ui.add(egui::Label::new(shown).truncate());
                });
                if ui.button(t("Choose Folder...")).clicked() {
                    let mut fd = rfd::FileDialog::new();
                    if let Some(f) = &dlg.folder {
                        fd = fd.set_directory(f);
                    }
                    if let Some(f) = fd.pick_folder() {
                        dlg.folder = Some(f);
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(t("Format"));
                    egui::ComboBox::from_id_salt("export_format").width(90.0).selected_text(dlg.format.label()).show_ui(ui, |ui| {
                        for f in ImgFormat::ALL {
                            ui.selectable_value(&mut dlg.format, f, f.label());
                        }
                    });
                    ui.weak(t(dlg.format.note()));
                });
                ui.weak(t("Files are named after the diagrams; existing files with the same name are replaced."));
                ui.separator();
                let n = dlg.chosen.iter().filter(|c| c.1).count();
                ui.horizontal(|ui| {
                    if ui.add_enabled(n > 0 && dlg.folder.is_some(), egui::Button::new(format!("{} ({n})", t("Export")))).clicked() {
                        go = true;
                    }
                    if ui.button(t("Cancel")).clicked() {
                        cancel = true;
                    }
                });
            });
        if go {
            let folder = dlg.folder.clone().unwrap();
            let ids: Vec<u64> = dlg.chosen.iter().filter(|c| c.1).map(|c| c.0).collect();
            self.settings.export_format = dlg.format.ext().to_string();
            self.settings.save();
            self.start_batch_export(ids, folder, dlg.format);
        } else if !(cancel || esc || !open) {
            self.export_dlg = Some(dlg);
        }
    }

    /// Queue the diagrams; work_jobs draws and saves one per frame.
    fn start_batch_export(&mut self, ids: Vec<u64>, folder: PathBuf, format: ImgFormat) {
        let Some(p) = self.project.as_ref() else { return };
        let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        for id in ids {
            let Some(d) = p.diagram(id) else { continue };
            let base = file_name_for(&d.name);
            let mut name = base.clone();
            let mut k = 2;
            while !used.insert(name.to_lowercase()) {
                name = format!("{base} ({k})");
                k += 1;
            }
            queue.push_back((id, folder.join(format!("{name}.{}", format.ext()))));
        }
        if queue.is_empty() {
            return;
        }
        self.commit_edit();
        self.menu = None;
        let total = queue.len();
        self.batch = Some(BatchExport { queue, total, done: 0, failed: 0, folder, wait: 3 });
        self.job_cancel = false;
    }

    /// The selection (with what sits inside selected containers, attached ports, joined
    /// interfaces and the lines among them), or the whole diagram when nothing is selected,
    /// put on the clipboard as a picture (twice the 100% size) for pasting into PowerPoint,
    /// Word and the like.
    fn copy_as_image(&mut self) {
        let lang = self.settings.language;
        let (Some(p), Some(d)) = (self.project.as_ref(), self.diag()) else { return };
        let mut sub = d.clone();
        let picked = !self.sel_shapes.is_empty() || !self.sel_conns.is_empty();
        if picked {
            let mut ids: std::collections::HashSet<u64> = self.sel_shapes.iter().copied().collect();
            for c in d.connectors.iter().filter(|c| self.sel_conns.contains(&c.id)) {
                ids.insert(c.from);
                ids.insert(c.to);
            }
            for s in d.shapes.iter().filter(|s| ids.contains(&s.id)).cloned().collect::<Vec<_>>() {
                if s.kind.is_container() {
                    for o in d.shapes.iter().filter(|o| o.id != s.id && s.contains_rect(o)) {
                        ids.insert(o.id);
                    }
                }
            }
            for s in d.shapes.iter().filter(|s| ids.contains(&s.id)).cloned().collect::<Vec<_>>() {
                ids.extend(attached_ports(d, &s));
                ids.extend(joined_interfaces(d, &s));
            }
            sub.shapes.retain(|s| ids.contains(&s.id));
            let kept_lines: std::collections::HashSet<u64> = d.connectors.iter().filter(|c| ids.contains(&c.from) && ids.contains(&c.to)).map(|c| c.id).collect();
            // a note anchored to a kept line stays too
            sub.connectors.retain(|c| kept_lines.contains(&c.id) || (ids.contains(&c.from) && kept_lines.contains(&c.to)));
        }
        if sub.shapes.is_empty() {
            self.status = tr(lang, "There is nothing to copy.").to_string();
            return;
        }
        let pic = crate::offscreen::diagram_picture(&sub, &p.options, &self.look(), &self.font_defs, 2.0, 6000.0);
        let (w, h) = pic.dimensions();
        let rgba: Vec<u8> = pic.pixels().flat_map(|px| [px[0], px[1], px[2], 255]).collect();
        let result = arboard::Clipboard::new().and_then(|mut cb| cb.set_image(arboard::ImageData { width: w as usize, height: h as usize, bytes: std::borrow::Cow::Owned(rgba) }));
        self.status = match result {
            Ok(()) => format!("{} ({w} x {h})", tr(lang, if picked { "The selection was copied as an image" } else { "The diagram was copied as an image" })),
            Err(e) => format!("{}: {e}", tr(lang, "Could not copy the image")),
        };
    }

    fn export_image(&mut self) {
        let Some(id) = self.active else { return };
        let name = self.diag().map(|d| d.name.clone()).unwrap_or_default();
        // the format last used comes first; the file's extension decides the format
        let last = ImgFormat::from_id(&self.settings.export_format);
        let mut fd = rfd::FileDialog::new();
        for f in std::iter::once(last).chain(ImgFormat::ALL.into_iter().filter(|f| *f != last)) {
            fd = fd.add_filter(format!("{} {}", f.label(), tr(self.settings.language, "image")), f.exts());
        }
        if let Some(mut p) = fd.set_file_name(format!("{}.{}", file_name_for(&name), last.ext())).save_file() {
            let format = match ImgFormat::from_path(&p) {
                Some(f) => f,
                None => {
                    let mut s = p.into_os_string();
                    s.push(format!(".{}", last.ext()));
                    p = PathBuf::from(s);
                    last
                }
            };
            self.settings.export_format = format.ext().to_string();
            self.settings.save();
            self.commit_edit();
            // drawn off screen at twice its 100% size, so the view doesn't move
            let ok = self.picture_of(id, 2.0, 8000.0).map(|pic| save_picture(&pic, &p)).unwrap_or(false);
            self.status = if ok { format!("Exported {}", p.display()) } else { format!("Could not export {}", p.display()) };
        }
    }

    // ---------- hit testing ----------

    /// 2 = strong hit (body), 1 = weak (a lifeline's dashed line), 0 = none
    fn shape_hit(cam: &Cam, s: &Shape, p: Pos2) -> u8 {
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
    fn over_plain_shape(d: &Diagram, cam: &Cam, p: Pos2) -> bool {
        d.shapes.iter().any(|s| !s.kind.is_container() && Self::shape_hit(cam, s, p) == 2)
    }

    fn hit_shape(&self, d: &Diagram, cam: &Cam, p: Pos2, allow_weak: bool) -> Option<u64> {
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

    fn hit_conn(&self, d: &Diagram, cam: &Cam, p: Pos2) -> Option<u64> {
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
    fn hit_any(&self, cam: &Cam, p: Pos2) -> Option<EditTarget> {
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

    // ---------- UI: menus ----------

    fn menu_item(&self, ui: &mut egui::Ui, label: &str, shortcut: Option<&KeyboardShortcut>, enabled: bool) -> bool {
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
    fn menus(&self, ctx: &egui::Context) -> Vec<(String, char, Vec<MItem>)> {
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
            c("AI Assistant", self.show_ai, Act::ToggleAi),
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
        vec![(t("Project"), 'P', project), (t("Diagram"), 'D', diagram), (t("View"), 'V', view), (t("Window"), 'W', window)]
    }

    fn run_act(&mut self, ctx: &egui::Context, act: Act) {
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
            Act::ToggleAi => self.show_ai = !self.show_ai,
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
        }
    }

    /// The menu bar: Project (Alt+P), Diagram (Alt+D), View (Alt+V), Window (Alt+W).
    fn menu_bar(&mut self, ui: &mut egui::Ui) {
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
    fn menu_popups(&mut self, ctx: &egui::Context) {
        let Some(state) = self.menu.clone() else { return };
        let menus = self.menus(ctx);
        let Some((_, _, items)) = menus.get(state.top) else {
            self.menu = None;
            return;
        };
        let mut pos = self.menu_top_rects[state.top].left_bottom();
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

    /// Keyboard for the menus: Alt+P/D/V/W (and F10) open them; arrows move,
    /// Enter/Space choose, Esc closes. Returns true while a menu is open.
    fn menu_keys(&mut self, ctx: &egui::Context) -> bool {
        for (k, i) in [(Key::P, 0usize), (Key::D, 1), (Key::V, 2), (Key::W, 3)] {
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

    fn open_menu_by_key(&mut self, ctx: &egui::Context, top: usize) {
        let menus = self.menus(ctx);
        let first = menus
            .get(top)
            .and_then(|m| m.2.iter().position(|it| matches!(it, MItem::Act { enabled: true, .. } | MItem::Sub { enabled: true, .. })))
            .unwrap_or(usize::MAX);
        self.menu = Some(MenuState { top, path: vec![first] });
    }

    fn open_project_options(&mut self, page: usize) {
        if let Some(p) = &self.project {
            self.proj_opts = Some(ProjectOptionsDlg {
                o: p.options.clone(),
                name: p.name.clone(),
                author: p.author.clone(),
                description: p.description.clone(),
                page,
            });
        }
    }

    fn open_switcher(&mut self) {
        let items = self.switcher_items();
        let n = items.len();
        if n == 0 {
            return;
        }
        let cur = items.iter().position(|id| Some(*id) == self.active).unwrap_or(0);
        self.switcher = Some(if n > 1 { (cur + 1) % n } else { 0 });
    }

    /// Open diagrams first (in tab order), then every other diagram.
    fn switcher_items(&self) -> Vec<u64> {
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

    fn ask_delete_diagram(&mut self, id: u64) {
        if self.settings.confirm_delete {
            self.confirm_delete = Some(id);
        } else {
            self.delete_diagram(id);
        }
    }

    // ---------- UI: toolbar ----------

    fn toolbar(&mut self, ui: &mut egui::Ui) {
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
            if ui.add(egui::Button::new(format!("✨ {}", tr(lang, "AI Assistant"))).selected(self.show_ai).frame(false)).on_hover_text(tr(lang, "Draw or change diagrams by talking to an AI agent installed on this computer")).clicked() {
                self.show_ai = !self.show_ai;
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

    // ---------- UI: left panes ----------

    fn left_pane(&mut self, ui: &mut egui::Ui) {
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

    fn navigator_by_type(&mut self, ui: &mut egui::Ui, project: &Project) {
        let lang = self.settings.language;
        egui::CollapsingHeader::new(format!("🗀 {}", project.name)).id_salt("nav_root").default_open(true).show(ui, |ui| {
            let groups: [(&str, Vec<DiagramKind>); 3] = [
                ("UML Diagrams", DiagramKind::ALL.iter().copied().filter(|k| !matches!(k, DiagramKind::Erd | DiagramKind::Network)).collect()),
                ("Database Modeling", vec![DiagramKind::Erd]),
                ("Network", vec![DiagramKind::Network]),
            ];
            for (group, kinds) in groups {
                egui::CollapsingHeader::new(tr(lang, group)).id_salt(group).default_open(true).show(ui, |ui| {
                    for kind in kinds {
                        let list: Vec<&Diagram> = project.diagrams.iter().filter(|d| d.kind == kind).collect();
                        let resp = egui::CollapsingHeader::new(format!("{} ({})", tr(lang, kind.label()), list.len()))
                            .id_salt(("kind", kind as u8))
                            .default_open(!list.is_empty())
                            .show(ui, |ui| {
                                for d in &list {
                                    let r = ui.selectable_label(self.active == Some(d.id), &d.name);
                                    if r.clicked() {
                                        self.open_diagram(d.id);
                                    }
                                    r.context_menu(|ui| {
                                        if ui.button(tr(lang, "Open")).clicked() {
                                            self.open_diagram(d.id);
                                            ui.close_menu();
                                        }
                                        if ui.button(tr(lang, "Rename")).clicked() {
                                            self.rename = Some((d.id, d.name.clone()));
                                            ui.close_menu();
                                        }
                                        if ui.button(tr(lang, "Duplicate Diagram")).clicked() {
                                            self.duplicate_diagram(d.id);
                                            ui.close_menu();
                                        }
                                        if ui.button(tr(lang, "Delete Diagram")).clicked() {
                                            self.ask_delete_diagram(d.id);
                                            ui.close_menu();
                                        }
                                    });
                                }
                            });
                        resp.header_response.context_menu(|ui| {
                            if ui.button(format!("{} {}", tr(lang, "New"), tr(lang, kind.label()))).clicked() {
                                self.add_diagram(kind, None);
                                ui.close_menu();
                            }
                        });
                    }
                });
            }
        });
    }

    /// Diagram Navigator: by folder (the person's own grouping) or by diagram type.
    fn navigator(&mut self, ui: &mut egui::Ui, project: &Project) {
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

    fn navigator_by_folder(&mut self, ui: &mut egui::Ui, project: &Project) {
        let lang = self.settings.language;
        let mut actions: Vec<NavAction> = vec![];
        let root = egui::CollapsingHeader::new(format!("🗀 {}", project.name)).id_salt("navf_root").default_open(true).show(ui, |ui| {
            self.folder_children(ui, project, None, 0, &mut actions);
        });
        let h = root.header_response;
        if h.clicked() {
            self.current_folder = None;
        }
        if let Some(p) = h.dnd_release_payload::<NavDrag>() {
            actions.push(NavAction::Move(*p, None));
        }
        if h.dnd_hover_payload::<NavDrag>().is_some() {
            ui.painter().rect_stroke(h.rect, 2.0, Stroke::new(1.5_f32, ui.visuals().selection.bg_fill), StrokeKind::Outside);
        }
        h.context_menu(|ui| {
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
        for a in actions {
            self.apply_nav(a);
        }
    }

    /// The folders and diagrams directly inside `parent` (None = the project root).
    /// `depth` is how many folders deep `parent` is (root 0); folders go 3 levels deep.
    fn folder_children(&mut self, ui: &mut egui::Ui, project: &Project, parent: Option<u64>, depth: usize, actions: &mut Vec<NavAction>) {
        let lang = self.settings.language;
        let accent = ui.visuals().selection.bg_fill;
        let mut folders: Vec<&Folder> = project.folders.iter().filter(|f| f.parent == parent).collect();
        folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        for f in folders {
            let resp = egui::CollapsingHeader::new(format!("🗀 {}", f.name)).id_salt(("folder", f.id)).default_open(true).show(ui, |ui| {
                self.folder_children(ui, project, Some(f.id), depth + 1, actions);
            });
            let h = resp.header_response.interact(Sense::drag());
            h.dnd_set_drag_payload(NavDrag::Folder(f.id));
            if let Some(p) = h.dnd_release_payload::<NavDrag>() {
                actions.push(NavAction::Move(*p, Some(f.id)));
            }
            if h.dnd_hover_payload::<NavDrag>().is_some() {
                ui.painter().rect_stroke(h.rect, 2.0, Stroke::new(1.5_f32, accent), StrokeKind::Outside);
            }
            if self.current_folder == Some(f.id) {
                ui.painter().rect_stroke(h.rect, 2.0, Stroke::new(1.0_f32, accent.linear_multiply(0.6)), StrokeKind::Inside);
            }
            if h.clicked() {
                self.current_folder = Some(f.id);
            }
            let fid = f.id;
            h.context_menu(|ui| {
                if ui.add_enabled(depth + 1 < MAX_FOLDER_DEPTH, egui::Button::new(tr(lang, "New Folder"))).clicked() {
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
        let mut diagrams: Vec<&Diagram> = project.diagrams.iter().filter(|d| d.folder == parent || (parent.is_none() && d.folder.map(|f| project.folder(f).is_none()).unwrap_or(false))).collect();
        diagrams.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        for d in diagrams {
            let r = ui.selectable_label(self.active == Some(d.id), format!("▫ {}", d.name)).on_hover_text(tr(lang, d.kind.label()));
            let r = r.interact(Sense::drag());
            r.dnd_set_drag_payload(NavDrag::Diagram(d.id));
            if r.clicked() {
                self.open_diagram(d.id);
                self.current_folder = parent;
            }
            let did = d.id;
            let dname = d.name.clone();
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
    }

    fn apply_nav(&mut self, a: NavAction) {
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

    fn explorer(&mut self, ui: &mut egui::Ui, project: &Project) {
        egui::CollapsingHeader::new(format!("🗀 {}", project.name)).id_salt("exp_root").default_open(true).show(ui, |ui| {
            for d in &project.diagrams {
                egui::CollapsingHeader::new(&d.name).id_salt(("exp", d.id)).default_open(self.active == Some(d.id)).show(ui, |ui| {
                    if d.shapes.is_empty() {
                        ui.weak("—");
                    }
                    for s in &d.shapes {
                        let kind_label = palette::label_for(d.kind, Tool::Shape(s.kind));
                        let name = if s.name.is_empty() { "(unnamed)" } else { s.name.as_str() };
                        let r = ui.selectable_label(self.active == Some(d.id) && self.sel_shapes.contains(&s.id), format!("{name}  ·  {kind_label}"));
                        if r.clicked() {
                            self.open_diagram(d.id);
                            self.sel_shapes = vec![s.id];
                            self.sel_conns.clear();
                            self.center_on(s.id);
                        }
                    }
                });
            }
        });
    }

    fn center_on(&mut self, id: u64) {
        let rect = self.canvas_rect;
        let Some((cx, cy)) = self.diag().and_then(|d| d.shape(id)).map(|s| s.center()) else { return };
        if let Some(v) = self.view_mut() {
            if rect.is_positive() {
                v.pan = Vec2::new(rect.width() / 2.0 - cx * v.zoom, rect.height() / 2.0 - cy * v.zoom);
            }
        }
    }

    // ---------- UI: property pane ----------

    fn property_pane(&mut self, ui: &mut egui::Ui) {
        let lang = self.settings.language;
        ui.horizontal(|ui| {
            ui.heading(tr(lang, "Property"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("»").frame(false)).on_hover_text(tr(lang, "Hide this pane")).clicked() {
                    self.show_property = false;
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let Some(d) = self.diag().cloned() else {
                ui.label(tr(lang, "No diagram is open."));
                return;
            };
            if self.sel_shapes.len() == 1 && self.sel_conns.is_empty() {
                if let Some(s) = d.shape(self.sel_shapes[0]).cloned() {
                    self.shape_properties(ui, &d, s);
                    return;
                }
            }
            if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
                if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]).cloned() {
                    self.conn_properties(ui, &d, c);
                    return;
                }
            }
            if !self.sel_shapes.is_empty() || !self.sel_conns.is_empty() {
                ui.label(format!("{} {}, {} {}", self.sel_shapes.len(), tr(lang, "elements"), self.sel_conns.len(), tr(lang, "connectors")));
                ui.add_space(6.0);
                ui.label(tr(lang, "Fill color"));
                let mut c = Color32::from_rgb(122, 207, 245);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    self.edit_checkpoint((0, 99));
                    let ids = self.sel_shapes.clone();
                    if let Some(dm) = self.diag_mut() {
                        for s in dm.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
                            s.fill = Some(c.to_array());
                        }
                    }
                }
                ui.weak(tr(lang, "Pastel colors"));
                if let Some(c) = pastel_swatches(ui, lang, None) {
                    self.checkpoint();
                    let ids = self.sel_shapes.clone();
                    if let Some(dm) = self.diag_mut() {
                        for s in dm.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
                            s.fill = Some(c);
                        }
                    }
                }
                ui.add_space(6.0);
                ui.label(tr(lang, "Font"));
                let mut pick: Option<Option<String>> = None;
                egui::ComboBox::from_id_salt("multi_font").selected_text("…").show_ui(ui, |ui| {
                    if ui.selectable_label(false, tr(lang, "(Default)")).clicked() {
                        pick = Some(None);
                    }
                    for f in crate::fonts::available() {
                        if ui.selectable_label(false, *f).clicked() {
                            pick = Some(Some(f.to_string()));
                        }
                    }
                });
                if let Some(font) = pick {
                    self.checkpoint();
                    let ids = self.sel_shapes.clone();
                    if let Some(dm) = self.diag_mut() {
                        for s in dm.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
                            s.font = font.clone();
                        }
                    }
                    self.fonts_check = true;
                }
                return;
            }
            // the diagram itself
            let mut name = d.name.clone();
            let mut doc = d.documentation.clone();
            let mut name_changed = false;
            egui::Grid::new("diag_props").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label(tr(lang, "Name"));
                if ui.text_edit_singleline(&mut name).changed() {
                    name_changed = true;
                }
                ui.end_row();
                ui.label(tr(lang, "Type"));
                ui.label(tr(lang, d.kind.label()));
                ui.end_row();
                ui.label(tr(lang, "elements"));
                ui.label(format!("{}", d.shapes.len()));
                ui.end_row();
                ui.label(tr(lang, "connectors"));
                ui.label(format!("{}", d.connectors.len()));
                ui.end_row();
            });
            if d.kind == DiagramKind::Sequence {
                self.diagram_numbering(ui, &d);
            }
            if name_changed {
                self.edit_checkpoint((d.id, 1));
                if let Some(dm) = self.diag_mut() {
                    dm.name = name;
                }
            }
            ui.add_space(6.0);
            ui.label(tr(lang, "Documentation"));
            if ui.add(egui::TextEdit::multiline(&mut doc).desired_rows(5).desired_width(f32::INFINITY)).changed() {
                self.edit_checkpoint((d.id, 2));
                if let Some(dm) = self.diag_mut() {
                    dm.documentation = doc;
                }
            }
        });
    }

    /// The sequence diagram's own numbering settings; "Project default" follows Project Options.
    fn diagram_numbering(&mut self, ui: &mut egui::Ui, d: &Diagram) {
        let lang = self.settings.language;
        let t = |s: &str| tr(lang, s).to_string();
        let o = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let mut numbered = d.number_messages;
        let mut format = d.sequence_numbering;
        let mut manual = d.manual_numbering;
        let on_off = |b: bool| if b { t("On") } else { t("Off") };
        let fmt_name = |f: u8| if f == crate::model::SEQ_SINGLE_LEVEL { t("Single level") } else { t("Nested level") };
        let hand_name = |m: bool| if m { t("Manual") } else { t("Automatic") };
        let default_of = |s: String| format!("{} ({s})", t("Project default"));
        ui.add_space(8.0);
        ui.label(egui::RichText::new(t("Sequence numbers")).strong());
        egui::Grid::new("diag_numbering").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(t("Number messages"));
            let shown = numbered.map(on_off).unwrap_or_else(|| default_of(on_off(o.number_messages)));
            egui::ComboBox::from_id_salt("diag_num_on").width(165.0).selected_text(shown).show_ui(ui, |ui| {
                ui.selectable_value(&mut numbered, None, default_of(on_off(o.number_messages)));
                ui.selectable_value(&mut numbered, Some(true), t("On"));
                ui.selectable_value(&mut numbered, Some(false), t("Off"));
            });
            ui.end_row();
            ui.label(t("Sequence number format"));
            let shown = format.map(fmt_name).unwrap_or_else(|| default_of(fmt_name(o.sequence_numbering)));
            egui::ComboBox::from_id_salt("diag_num_fmt").width(165.0).selected_text(shown).show_ui(ui, |ui| {
                ui.selectable_value(&mut format, None, default_of(fmt_name(o.sequence_numbering)));
                ui.selectable_value(&mut format, Some(crate::model::SEQ_NESTED_LEVEL), format!("{} (1, 1.1, 1.2.1)", fmt_name(crate::model::SEQ_NESTED_LEVEL)));
                ui.selectable_value(&mut format, Some(crate::model::SEQ_SINGLE_LEVEL), format!("{} (1, 2, 3)", fmt_name(crate::model::SEQ_SINGLE_LEVEL)));
            });
            ui.end_row();
            ui.label(t("Sequence number handling"));
            let shown = manual.map(hand_name).unwrap_or_else(|| default_of(hand_name(o.manual_numbering)));
            egui::ComboBox::from_id_salt("diag_num_hand").width(165.0).selected_text(shown).show_ui(ui, |ui| {
                ui.selectable_value(&mut manual, None, default_of(hand_name(o.manual_numbering)));
                ui.selectable_value(&mut manual, Some(false), t("Automatic"));
                ui.selectable_value(&mut manual, Some(true), t("Manual"));
            });
            ui.end_row();
        });
        if (numbered, format, manual) != (d.number_messages, d.sequence_numbering, d.manual_numbering) {
            self.checkpoint();
            if let Some(dm) = self.diag_mut() {
                dm.number_messages = numbered;
                dm.sequence_numbering = format;
                dm.manual_numbering = manual;
            }
        }
        ui.weak(t("These settings apply to this diagram only; Project default follows Window > Project Options."));
    }

    fn shape_properties(&mut self, ui: &mut egui::Ui, d: &Diagram, mut s: Shape) {
        let lang = self.settings.language;
        let default_fill = self.look().fill(s.kind).to_array();
        let default_size = self.settings.text_size;
        let mut field_changed: Option<u8> = None;
        ui.label(egui::RichText::new(palette::label_for(d.kind, Tool::Shape(s.kind))).strong());
        ui.add_space(4.0);
        egui::Grid::new("shape_props").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            // notes and free text keep their line breaks, so they get a multi-line box
            let multi = matches!(s.kind, ShapeKind::Note | ShapeKind::Text);
            ui.label(tr(lang, if s.kind == ShapeKind::Text { "Text" } else { "Name" }));
            let te = if multi {
                egui::TextEdit::multiline(&mut s.name).desired_rows(4).desired_width(f32::INFINITY)
            } else {
                egui::TextEdit::singleline(&mut s.name).desired_width(f32::INFINITY)
            };
            if ui.add(te).changed() {
                field_changed = Some(1);
            }
            ui.end_row();
            if s.kind == ShapeKind::Text {
                let aligns = ["Left", "Center", "Right"];
                ui.label(tr(lang, "Alignment"));
                egui::ComboBox::from_id_salt("text_align").selected_text(tr(lang, aligns[(s.orient % 3) as usize])).show_ui(ui, |ui| {
                    for (i, l) in aligns.iter().enumerate() {
                        if ui.selectable_label(s.orient % 3 == i as u8, tr(lang, l)).clicked() {
                            s.orient = i as u8;
                            field_changed = Some(11);
                        }
                    }
                });
                ui.end_row();
            }
            if (!s.kind.is_pseudo() || s.kind == ShapeKind::Actor || s.kind.is_network_device()) && s.kind != ShapeKind::Text {
                ui.label(tr(lang, "Stereotype"));
                if ui.add(egui::TextEdit::singleline(&mut s.stereotype).desired_width(f32::INFINITY)).changed() {
                    field_changed = Some(2);
                }
                ui.end_row();
            }
            ui.label("X / Y");
            ui.horizontal(|ui| {
                if ui.add(egui::DragValue::new(&mut s.x).speed(1.0)).changed() {
                    field_changed = Some(3);
                }
                if ui.add(egui::DragValue::new(&mut s.y).speed(1.0)).changed() {
                    field_changed = Some(3);
                }
            });
            ui.end_row();
            if !s.kind.fixed_size() {
                ui.label("W / H");
                ui.horizontal(|ui| {
                    if ui.add(egui::DragValue::new(&mut s.w).speed(1.0).range(8.0..=4000.0)).changed() {
                        field_changed = Some(4);
                    }
                    if ui.add(egui::DragValue::new(&mut s.h).speed(1.0).range(4.0..=4000.0)).changed() {
                        field_changed = Some(4);
                    }
                });
                ui.end_row();
            }
            ui.label(tr(lang, "Fill color"));
            ui.horizontal(|ui| {
                let mut def = s.fill.is_none();
                if ui.checkbox(&mut def, tr(lang, "Default")).changed() {
                    s.fill = if def { None } else { Some(default_fill) };
                    field_changed = Some(5);
                }
                if let Some(f) = s.fill {
                    let mut c = Color32::from_rgba_unmultiplied(f[0], f[1], f[2], f[3]);
                    if ui.color_edit_button_srgba(&mut c).changed() {
                        s.fill = Some(c.to_array());
                        field_changed = Some(5);
                    }
                }
            });
            ui.end_row();
            ui.label(tr(lang, "Font"));
            let cur = s.font.clone().unwrap_or_default();
            let shown = if cur.is_empty() { tr(lang, "(Default)").to_string() } else { cur.clone() };
            egui::ComboBox::from_id_salt("shape_font").selected_text(shown).show_ui(ui, |ui| {
                if ui.selectable_label(cur.is_empty(), tr(lang, "(Default)")).clicked() {
                    s.font = None;
                    field_changed = Some(9);
                }
                for f in crate::fonts::available() {
                    if ui.selectable_label(cur == *f, *f).clicked() {
                        s.font = Some(f.to_string());
                        field_changed = Some(9);
                    }
                }
            });
            ui.end_row();
            ui.label(tr(lang, "Text size"));
            ui.horizontal(|ui| {
                let mut def = s.font_size.is_none();
                if ui.checkbox(&mut def, tr(lang, "Default")).changed() {
                    s.font_size = if def { None } else { Some(default_size) };
                    field_changed = Some(10);
                }
                if let Some(mut sz) = s.font_size {
                    if ui.add(egui::DragValue::new(&mut sz).range(6.0..=48.0).speed(0.2)).changed() {
                        s.font_size = Some(sz);
                        field_changed = Some(10);
                    }
                }
            });
            ui.end_row();
            if s.kind == ShapeKind::Component {
                ui.label(tr(lang, "Component icon"));
                let mut hide = s.hide_icon;
                if ui.checkbox(&mut hide, tr(lang, "Hide")).changed() {
                    s.hide_icon = hide;
                    field_changed = Some(12);
                }
                ui.end_row();
            }
            if s.kind == ShapeKind::Class {
                // per-class: hide the attribute and/or operation compartment
                ui.label(tr(lang, "Attributes"));
                let mut hide = s.hide_attributes;
                if ui.checkbox(&mut hide, tr(lang, "Hide")).changed() {
                    s.hide_attributes = hide;
                    field_changed = Some(14);
                }
                ui.end_row();
                ui.label(tr(lang, "Operations"));
                let mut hide = s.hide_operations;
                if ui.checkbox(&mut hide, tr(lang, "Hide")).changed() {
                    s.hide_operations = hide;
                    field_changed = Some(14);
                }
                ui.end_row();
            }
            if s.kind == ShapeKind::RequiredInterface {
                let dirs = ["Right", "Down", "Left", "Up"];
                ui.label(tr(lang, "Opening"));
                egui::ComboBox::from_id_salt("orient").selected_text(tr(lang, dirs[(s.orient % 4) as usize])).show_ui(ui, |ui| {
                    for (i, l) in dirs.iter().enumerate() {
                        if ui.selectable_label(s.orient % 4 == i as u8, tr(lang, l)).clicked() {
                            s.orient = i as u8;
                            field_changed = Some(11);
                        }
                    }
                });
                ui.end_row();
            }
        });
        if !matches!(s.kind, ShapeKind::Initial | ShapeKind::Final | ShapeKind::ActivityFinal | ShapeKind::Junction | ShapeKind::Fork | ShapeKind::Join) {
            ui.add_space(4.0);
            ui.weak(tr(lang, "Pastel colors"));
            if let Some(c) = pastel_swatches(ui, lang, s.fill) {
                s.fill = Some(c);
                field_changed = Some(5);
            }
        }
        if s.kind.is_swimlane() {
            // the partitions, in order: rename, remove, add
            ui.add_space(6.0);
            ui.label(tr(lang, "Partitions"));
            let mut v = s.lane_partitions();
            let n = v.len();
            let mut remove: Option<usize> = None;
            let mut add = false;
            let mut typed = false;
            for (i, p) in v.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}", i + 1));
                    if ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(160.0).hint_text(tr(lang, "Partition name"))).changed() {
                        typed = true;
                    }
                    if ui.add_enabled(n > 1, egui::Button::new("×").small()).on_hover_text(tr(lang, "Remove Partition")).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if ui.button(tr(lang, "Add Partition")).clicked() {
                add = true;
            }
            ui.weak(tr(lang, "Drag the line between two partitions to change their sizes; double-click a partition's header to rename it."));
            if typed {
                let was_old = s.partitions.is_none();
                s.normalize_partitions();
                if let Some(cur) = s.partitions.as_mut() {
                    for (c, p) in cur.iter_mut().zip(v.iter()) {
                        c.name = p.name.clone();
                    }
                }
                let _ = was_old;
                field_changed = Some(17);
            }
            if let Some(i) = remove {
                self.remove_partition(s.id, i);
                return;
            }
            if add {
                self.add_partition(s.id, n.saturating_sub(1));
                return;
            }
        }
        if s.kind == ShapeKind::AltFragment {
            // the guard of each section below the first (whose guard is the name)
            ui.add_space(6.0);
            ui.label(tr(lang, "Sections"));
            let mut v = s.alt_dividers();
            let mut remove: Option<usize> = None;
            let mut add = false;
            let mut typed = false;
            for (i, dv) in v.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}", i + 2));
                    if ui.add(egui::TextEdit::singleline(&mut dv.guard).desired_width(160.0).hint_text(tr(lang, "guard"))).changed() {
                        typed = true;
                    }
                    if ui.small_button("×").on_hover_text(tr(lang, "Remove Else Section")).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if ui.button(tr(lang, "Add Else Section")).clicked() {
                add = true;
            }
            ui.weak(tr(lang, "Drag a section line on the diagram to change the sections' sizes."));
            if typed {
                s.dividers = Some(v.clone());
                field_changed = Some(15);
            }
            if let Some(i) = remove {
                v.remove(i);
                s.dividers = Some(v.clone());
                field_changed = Some(16);
            }
            if add {
                self.add_alt_section(s.id);
                return;
            }
        }
        if render::has_name_below(s.kind) && s.label_offset != (0.0, 0.0) && ui.button(tr(lang, "Reset Name Position")).clicked() {
            s.label_offset = (0.0, 0.0);
            field_changed = Some(13);
        }
        if s.kind.has_attributes() {
            ui.add_space(6.0);
            ui.label(tr(lang, s.kind.attributes_label()));
            let mut text = s.attributes.join("\n");
            if ui.add(egui::TextEdit::multiline(&mut text).desired_rows(4).desired_width(f32::INFINITY).code_editor()).changed() {
                s.attributes = text.split('\n').map(|l| l.to_string()).collect();
                field_changed = Some(6);
            }
            let hint = if matches!(s.kind, ShapeKind::Entity | ShapeKind::DbView) {
                format!("{} {}", tr(lang, "One item per line."), tr(lang, "Start a line with PK for a primary key."))
            } else {
                tr(lang, "One item per line.").to_string()
            };
            ui.weak(hint);
        }
        if s.kind.has_operations() {
            ui.add_space(6.0);
            ui.label(tr(lang, "Operations"));
            let mut text = s.operations.join("\n");
            if ui.add(egui::TextEdit::multiline(&mut text).desired_rows(3).desired_width(f32::INFINITY).code_editor()).changed() {
                s.operations = text.split('\n').map(|l| l.to_string()).collect();
                field_changed = Some(7);
            }
        }
        ui.add_space(6.0);
        ui.label(tr(lang, "Documentation"));
        if ui.add(egui::TextEdit::multiline(&mut s.documentation).desired_rows(4).desired_width(f32::INFINITY)).changed() {
            field_changed = Some(8);
        }
        if let Some(f) = field_changed {
            if f == 9 {
                self.fonts_check = true;
            }
            if f == 14 {
                // hiding or showing a compartment fits the class height to what is left
                let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                s.h = render::min_size(ui.ctx(), &s, &opts, &self.look()).y.max(30.0);
            }
            self.edit_checkpoint((s.id, f));
            if let Some(dm) = self.diag_mut() {
                if let Some(t) = dm.shape_mut(s.id) {
                    *t = s;
                }
            }
        }
    }

    fn conn_properties(&mut self, ui: &mut egui::Ui, d: &Diagram, mut c: Connector) {
        let lang = self.settings.language;
        let mut changed: Option<u8> = None;
        let mut reverse = false;
        ui.label(egui::RichText::new(palette::label_for(d.kind, Tool::Conn(c.kind))).strong());
        ui.add_space(4.0);
        let from = d.shape(c.from).map(|s| s.name.clone()).unwrap_or_default();
        let to = d.shape(c.to).map(|s| s.name.clone()).unwrap_or_default();
        egui::Grid::new("conn_props").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(tr(lang, "Name"));
            if ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(f32::INFINITY)).changed() {
                changed = Some(1);
            }
            ui.end_row();
            ui.label(tr(lang, "Source"));
            ui.label(from);
            ui.end_row();
            ui.label(tr(lang, "Target"));
            ui.label(to);
            ui.end_row();
            let manual = self.project.as_ref().map(|p| d.manual(&p.options)).unwrap_or(false);
            if c.kind.is_message() && d.kind == DiagramKind::Sequence && manual {
                ui.label(tr(lang, "Sequence number"));
                if ui.add(egui::TextEdit::singleline(&mut c.seq_no).desired_width(f32::INFINITY).hint_text(tr(lang, "Automatic"))).changed() {
                    changed = Some(9);
                }
                ui.end_row();
            }
            if !c.kind.is_message() {
                ui.label(tr(lang, "Source label"));
                if ui.add(egui::TextEdit::singleline(&mut c.from_label).desired_width(f32::INFINITY)).changed() {
                    changed = Some(2);
                }
                ui.end_row();
                ui.label(tr(lang, "Target label"));
                if ui.add(egui::TextEdit::singleline(&mut c.to_label).desired_width(f32::INFINITY)).changed() {
                    changed = Some(3);
                }
                ui.end_row();
            }
            // switch between the connector kinds this diagram offers
            let kinds: Vec<(ConnKind, &str)> = palette::palette(d.kind)
                .into_iter()
                .filter_map(|e| if let Tool::Conn(k) = e.tool { Some((k, e.label)) } else { None })
                .collect();
            ui.label(tr(lang, "Type"));
            let cur = palette::label_for(d.kind, Tool::Conn(c.kind));
            egui::ComboBox::from_id_salt("conn_kind").selected_text(cur).show_ui(ui, |ui| {
                for (k, l) in kinds {
                    if ui.selectable_label(k == c.kind, l).clicked() && k != c.kind {
                        c.kind = k;
                        changed = Some(4);
                    }
                }
            });
            ui.end_row();
        });
        if navigable(c.kind) {
            use crate::model::{NAV_FALSE, NAV_TRUE, NAV_UNSPECIFIED};
            let choices = [(NAV_TRUE, "True"), (NAV_FALSE, "False"), (NAV_UNSPECIFIED, "Unspecified")];
            egui::Grid::new("nav_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                for (label, target) in [("Source navigable", false), ("Target navigable", true)] {
                    ui.label(tr(lang, label));
                    let cur = if target { c.nav_to } else { c.nav_from };
                    let shown = choices.iter().find(|(v, _)| *v == cur).map(|(_, n)| *n).unwrap_or("Unspecified");
                    egui::ComboBox::from_id_salt(("nav", target)).selected_text(tr(lang, shown)).show_ui(ui, |ui| {
                        for (v, n) in choices {
                            if ui.selectable_label(cur == v, tr(lang, n)).clicked() {
                                if target {
                                    c.nav_to = v;
                                } else {
                                    c.nav_from = v;
                                }
                                c.arrow_end = false;
                                c.arrow_start = false;
                                changed = Some(6);
                            }
                        }
                    });
                    ui.end_row();
                }
            });
            ui.weak(tr(lang, "An arrow shows at a navigable end when the other end is not navigable."));
        }
        if !(d.kind == DiagramKind::Sequence && c.kind.is_message()) {
            ui.horizontal(|ui| {
                ui.label(tr(lang, "Connector Style"));
                let shown = render::LINE_STYLES.iter().find(|(v, _)| *v == c.style).map(|(_, n)| *n).unwrap_or("Follow diagram");
                egui::ComboBox::from_id_salt("conn_style").selected_text(tr(lang, shown)).show_ui(ui, |ui| {
                    for (v, n) in render::LINE_STYLES {
                        if ui.selectable_label(c.style == v, tr(lang, n)).clicked() {
                            c.style = v;
                            changed = Some(8);
                        }
                    }
                });
            });
        }
        let mut line_action: Option<bool> = None;
        if !(d.kind == DiagramKind::Sequence && c.kind.is_message()) {
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "Straight Line")).clicked() {
                    line_action = Some(false);
                }
                if ui.button(tr(lang, "Right-Angle Line")).clicked() {
                    line_action = Some(true);
                }
            });
            ui.weak(tr(lang, "Drag the dots on the selected line to bend it; double-click a square to remove a bend."));
        }
        if ui.button(tr(lang, "Reverse Direction")).clicked() {
            reverse = true;
        }
        if c.label_offset != (0.0, 0.0) && ui.button(tr(lang, "Reset Name Position")).clicked() {
            c.label_offset = (0.0, 0.0);
            changed = Some(10);
        }
        if (c.from_label_offset != (0.0, 0.0) || c.to_label_offset != (0.0, 0.0)) && ui.button(tr(lang, "Reset End Label Positions")).clicked() {
            c.from_label_offset = (0.0, 0.0);
            c.to_label_offset = (0.0, 0.0);
            changed = Some(11);
        }
        ui.add_space(6.0);
        ui.label(tr(lang, "Documentation"));
        if ui.add(egui::TextEdit::multiline(&mut c.documentation).desired_rows(4).desired_width(f32::INFINITY)).changed() {
            changed = Some(5);
        }
        if let Some(right) = line_action {
            self.set_line_shape(c.id, right);
            return;
        }
        if reverse {
            std::mem::swap(&mut c.from, &mut c.to);
            std::mem::swap(&mut c.arrow_start, &mut c.arrow_end);
            c.points.reverse();
            std::mem::swap(&mut c.from_label, &mut c.to_label);
            std::mem::swap(&mut c.from_label_offset, &mut c.to_label_offset);
            self.checkpoint();
            if let Some(dm) = self.diag_mut() {
                if let Some(t) = dm.conn_mut(c.id) {
                    *t = c;
                }
            }
        } else if let Some(f) = changed {
            self.edit_checkpoint((c.id, f + 20));
            if let Some(dm) = self.diag_mut() {
                if let Some(t) = dm.conn_mut(c.id) {
                    *t = c;
                }
            }
        }
    }

    // ---------- UI: palette (diagram toolbar) ----------

    fn palette_row(&mut self, ui: &mut egui::Ui, tool: Tool, label: &str, tip: &str, look: &Look, opts: &ProjectOptions) {
        let labels = self.settings.palette_labels;
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
        let selected = self.tool == tool;
        if selected {
            ui.painter().rect_filled(rect, 3.0, ui.visuals().selection.bg_fill.linear_multiply(0.3));
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let icon = Rect::from_min_size(rect.min + Vec2::new(4.0, 3.0), Vec2::splat(18.0));
        render::draw_icon(ui.painter(), icon, tool, look, opts);
        if labels {
            ui.painter().text(Pos2::new(icon.right() + 6.0, rect.center().y), Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0), ui.visuals().text_color());
        }
        let selected_now = self.tool == tool;
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected_now, label));
        let resp = resp.on_hover_text(if tip.is_empty() { label.to_string() } else { format!("{label}\n{tip}") });
        if resp.clicked() {
            self.tool = tool;
            self.sticky = false;
        }
        if resp.double_clicked() {
            self.tool = tool;
            self.sticky = true;
        }
    }

    fn palette_pane(&mut self, ui: &mut egui::Ui, kind: DiagramKind) {
        let look = self.look();
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let entries = palette::palette(kind);
        let select_label = self.t("Select").to_string();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            self.palette_row(ui, Tool::Select, &select_label, "", &look, &opts);
            ui.separator();
            let mut seen_conn = false;
            for e in entries {
                if matches!(e.tool, Tool::Conn(_)) && !seen_conn {
                    seen_conn = true;
                    ui.separator();
                }
                self.palette_row(ui, e.tool, e.label, e.tip, &look, &opts);
            }
        });
    }

    // ---------- UI: canvas ----------

    fn canvas(&mut self, ui: &mut egui::Ui) {
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

        self.canvas_input(ui, &resp, &cam, &d);

        // previews
        match &self.drag {
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
                    }
                }
            }
            _ => {}
        }
        if let Tool::Shape(_) | Tool::Conn(_) = self.tool {
            let label = palette::label_for(d.kind, self.tool);
            painter.text(rect.left_bottom() + Vec2::new(8.0, -8.0), Align2::LEFT_BOTTOM, format!("✚ {label}"), egui::FontId::proportional(12.0), look.accent());
        }

        self.inline_editor(ui.ctx(), &cam);
    }

    fn canvas_input(&mut self, ui: &mut egui::Ui, resp: &egui::Response, cam: &Cam, d: &Diagram) {
        if self.screenshot.is_some() {
            // a check run must not react to the person's mouse
            return;
        }
        let ctx = ui.ctx().clone();
        let pointer = ctx.input(|i| i.pointer.interact_pos()).or(resp.hover_pos());
        let shift = ctx.input(|i| i.modifiers.shift || i.modifiers.command);

        // zoom and scroll
        if resp.hovered() {
            let zd = ctx.input(|i| i.zoom_delta());
            if (zd - 1.0).abs() > 1e-4 {
                // one wheel notch = one Zoom In / Zoom Out step
                if zd > 1.0 {
                    self.zoom_in_step(pointer);
                } else {
                    self.zoom_out_step(pointer);
                }
            } else {
                let sd = ctx.input(|i| i.smooth_scroll_delta);
                if sd != Vec2::ZERO {
                    if let Some(v) = self.view_mut() {
                        v.pan += sd;
                    }
                }
            }
        }
        if resp.dragged_by(PointerButton::Middle) || resp.dragged_by(PointerButton::Secondary) {
            let delta = resp.drag_delta();
            if let Some(v) = self.view_mut() {
                v.pan += delta;
            }
        }

        // start of a primary drag
        if resp.drag_started_by(PointerButton::Primary) {
            let origin = ctx.input(|i| i.pointer.press_origin()).unwrap_or_default();
            let dp = cam.to_diagram(origin);
            self.commit_edit();
            match self.tool {
                Tool::Conn(kind) => {
                    if kind == ConnKind::Anchor {
                        // a note can be attached to a shape or to a line (e.g. a message)
                        if let Some(EditTarget::Shape(id) | EditTarget::Conn(id)) = self.hit_any(cam, origin) {
                            self.drag = Drag::Connect { from: id, start: dp, cur: origin };
                        }
                    } else if let Some(id) = self.hit_shape(d, cam, origin, true) {
                        self.drag = Drag::Connect { from: id, start: dp, cur: origin };
                    }
                }
                Tool::Shape(_) => {}
                Tool::Select => {
                    let mut handled = false;
                    // a section line of an alternative fragment?
                    if !handled {
                        if let Some((id, idx, y)) = divider_at(d, cam, origin) {
                            self.checkpoint();
                            if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
                                s.dividers = Some(s.alt_dividers());
                            }
                            self.clear_selection();
                            self.sel_shapes.push(id);
                            self.drag = Drag::Divider { id, idx, orig: y };
                            handled = true;
                        }
                    }
                    // the line between two partitions of a swimlane (not over a shape on it)?
                    if !handled && !Self::over_plain_shape(d, cam, origin) {
                        if let Some((id, idx)) = partition_edge_at(d, cam, origin) {
                            self.checkpoint();
                            let mut vertical = true;
                            let mut orig = vec![];
                            if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
                                s.normalize_partitions();
                                vertical = s.kind == ShapeKind::VerticalSwimlane;
                                orig = s.lane_partitions().iter().map(|p| p.size).collect();
                            }
                            self.clear_selection();
                            self.sel_shapes.push(id);
                            self.drag = Drag::PartEdge { id, idx, vertical, orig };
                            handled = true;
                        }
                    }
                    // the top or bottom handle of the selected activation?
                    if let Some((r, top, bottom)) = self.bar_handles(d, cam) {
                        let pick = if (origin - top).length() < 8.0 { Some(true) } else if (origin - bottom).length() < 8.0 { Some(false) } else { None };
                        if let (Some(is_top), Some((lid, depth, _))) = (pick, self.sel_bar) {
                            if let Some(s) = d.shape(lid) {
                                self.checkpoint();
                                let (y0, y1) = (cam.to_diagram(r.left_top()).1, cam.to_diagram(r.left_bottom()).1);
                                self.drag = Drag::BarEdge { lifeline: lid, depth, top: is_top, y0, y1, splits0: s.splits.clone(), spans0: s.act_spans.clone() };
                                handled = true;
                            }
                        }
                    }
                    // an end of the selected line?
                    if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
                        if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                            if let Some(ends) = self.end_handles(d, cam, c) {
                                if let Some(k) = ends.iter().position(|e| (*e - origin).length() < 8.0) {
                                    self.checkpoint();
                                    self.drag = Drag::MoveEnd { id: c.id, to_end: k == 1 };
                                    handled = true;
                                }
                            }
                        }
                    }
                    // a bend point (or a segment's middle dot) of the selected line?
                    if !handled && self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
                        if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                            if let Some(hit) = Self::point_handle_hit(d, cam, c, origin) {
                                self.checkpoint();
                                let cid = c.id;
                                let idx = match hit {
                                    PointHit::Point(i) => i,
                                    PointHit::Mid(i) => {
                                        if let Some(cm) = self.diag_mut().and_then(|dm| dm.conn_mut(cid)) {
                                            cm.points.insert(i, dp);
                                        }
                                        i
                                    }
                                };
                                self.drag = Drag::MovePoint { id: cid, idx };
                                handled = true;
                            }
                        }
                    }
                    // a resize handle of the single selected shape?
                    if self.sel_shapes.len() == 1 {
                        if let Some(s) = d.shape(self.sel_shapes[0]) {
                            if !s.kind.fixed_size() {
                                for (i, h) in render::handle_points(cam, s).iter().enumerate() {
                                    if (origin - *h).length() < 7.0 {
                                        self.checkpoint();
                                        self.drag = Drag::Resize { id: s.id, corner: i, orig: (s.x, s.y, s.w, s.h), start: dp };
                                        handled = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // a name label (of a line, or below a small shape)?
                    if !handled {
                        if let Some(t) = self.label_at(origin) {
                            let orig = match t {
                                LabelTarget::Conn(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.label_offset),
                                LabelTarget::ConnFrom(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.from_label_offset),
                                LabelTarget::ConnTo(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.to_label_offset),
                                LabelTarget::Shape(id) => d.shape(id).map(|s| s.label_offset),
                            };
                            if let Some(orig) = orig {
                                self.clear_selection();
                                match t.owner() {
                                    EditTarget::Conn(id) => self.sel_conns.push(id),
                                    EditTarget::Shape(id) => self.sel_shapes.push(id),
                                }
                                self.checkpoint();
                                self.drag = Drag::MoveLabel { target: t, start: dp, orig };
                                handled = true;
                            }
                        }
                    }
                    if !handled {
                        match self.hit_any(cam, origin) {
                            Some(EditTarget::Shape(id)) => {
                                if !self.sel_shapes.contains(&id) {
                                    if !shift {
                                        self.clear_selection();
                                    }
                                    self.sel_shapes.push(id);
                                }
                                self.checkpoint();
                                let mut ids: Vec<u64> = self.sel_shapes.clone();
                                // containers carry what is inside them
                                for sid in self.sel_shapes.clone() {
                                    if let Some(cs) = d.shape(sid) {
                                        if cs.kind.is_container() {
                                            for o in &d.shapes {
                                                if o.id != cs.id && cs.contains_rect(o) && !ids.contains(&o.id) {
                                                    ids.push(o.id);
                                                }
                                            }
                                        }
                                    }
                                }
                                // ports attached to a moved shape move with it
                                for sid in ids.clone() {
                                    if let Some(cs) = d.shape(sid) {
                                        for pid in attached_ports(d, cs) {
                                            if !ids.contains(&pid) {
                                                ids.push(pid);
                                            }
                                        }
                                    }
                                }
                                // a joined ball and socket move as one
                                for sid in ids.clone() {
                                    if let Some(cs) = d.shape(sid) {
                                        for jid in joined_interfaces(d, cs) {
                                            if !ids.contains(&jid) {
                                                ids.push(jid);
                                            }
                                        }
                                    }
                                }
                                let items = ids.iter().filter_map(|i| d.shape(*i)).map(|s| (s.id, s.x, s.y)).collect();
                                // bent lines between moved shapes move along
                                let lines = d
                                    .connectors
                                    .iter()
                                    .filter(|c| !c.points.is_empty() && ids.contains(&c.from) && ids.contains(&c.to))
                                    .map(|c| (c.id, c.points.clone()))
                                    .collect();
                                self.drag = Drag::Move { start: dp, items, lines };
                            }
                            Some(EditTarget::Conn(id)) => {
                                if !shift {
                                    self.clear_selection();
                                }
                                if !self.sel_conns.contains(&id) {
                                    self.sel_conns.push(id);
                                }
                                if let Some(c) = d.connectors.iter().find(|c| c.id == id) {
                                    if d.kind == DiagramKind::Sequence && c.kind.is_message() {
                                        self.checkpoint();
                                        self.drag = Drag::MoveMessage { id, orig: c.y, start: dp.1 };
                                    } else if c.from != c.to && d.shape(c.from).is_some() && d.shape(c.to).is_some() {
                                        // drag the whole line: both ends slide along their shapes
                                        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                                        let (_, paths) = crate::seq::all_paths(d, &opts);
                                        if let Some(p) = paths.get(&id) {
                                            if let (Some(&f0), Some(&t0)) = (p.first(), p.last()) {
                                                self.checkpoint();
                                                self.drag = Drag::MoveLine { id, start: dp, from0: f0, to0: t0, points0: c.points.clone() };
                                            }
                                        }
                                    }
                                }
                            }
                            None => {
                                if !shift {
                                    self.clear_selection();
                                }
                                self.drag = Drag::Rubber { start: origin, cur: origin };
                            }
                        }
                    }
                }
            }
        }

        // while dragging
        if resp.dragged_by(PointerButton::Primary) {
            if let Some(p) = pointer {
                let dp = cam.to_diagram(p);
                let snap = self.settings.snap;
                let g = self.settings.grid_size.max(1.0);
                let snapv = move |v: f32| if snap { (v / g).round() * g } else { v };
                let mut moves: Vec<(u64, f32, f32)> = vec![];
                let mut resize: Option<(u64, f32, f32, f32, f32)> = None;
                let mut msg: Option<(u64, f32)> = None;
                let mut line_moves: Vec<(u64, Vec<(f32, f32)>)> = vec![];
                let mut point: Option<(u64, usize, (f32, f32))> = None;
                let mut label_move: Option<(LabelTarget, (f32, f32))> = None;
                let mut end_move: Option<(u64, bool)> = None;
                let mut line_move: Option<(u64, (f32, f32), (f32, f32), Vec<(f32, f32)>)> = None;
                let mut bar_edit: Option<(u64, usize, bool, f32, f32, Vec<f32>, Vec<[f32; 3]>, f32)> = None;
                let mut divider: Option<(u64, usize, f32)> = None;
                let mut part_edge: Option<(u64, usize, bool, (f32, f32))> = None;
                let locked: std::collections::HashSet<u64> = if matches!(self.drag, Drag::Move { .. }) { d.shapes.iter().filter(|s| y_locked(&d, s)).map(|s| s.id).collect() } else { Default::default() };
                match &mut self.drag {
                    Drag::Move { start, items, lines } => {
                        let (dx, dy) = (snapv(dp.0 - start.0), snapv(dp.1 - start.1));
                        // lifelines along the top of a sequence diagram move only sideways
                        moves = items.iter().map(|(id, x, y)| (*id, x + dx, if locked.contains(id) { *y } else { y + dy })).collect();
                        line_moves = lines.iter().map(|(id, pts)| (*id, pts.iter().map(|(x, y)| (x + dx, y + dy)).collect())).collect();
                    }
                    Drag::MovePoint { id, idx } => point = Some((*id, *idx, (snapv(dp.0), snapv(dp.1)))),
                    Drag::MoveLabel { target, start, orig } => label_move = Some((*target, (orig.0 + dp.0 - start.0, orig.1 + dp.1 - start.1))),
                    Drag::MoveEnd { id, to_end } => end_move = Some((*id, *to_end)),
                    Drag::MoveLine { id, start, from0, to0, points0 } => {
                        let (dx, dy) = (snapv(dp.0 - start.0), snapv(dp.1 - start.1));
                        line_move = Some((*id, (from0.0 + dx, from0.1 + dy), (to0.0 + dx, to0.1 + dy), points0.iter().map(|(x, y)| (x + dx, y + dy)).collect()));
                    }
                    Drag::Resize { id, corner, orig, start } => {
                        let (dx, dy) = (dp.0 - start.0, dp.1 - start.1);
                        let (ox, oy, ow, oh) = *orig;
                        let (mut x0, mut y0, mut x1, mut y1) = (ox, oy, ox + ow, oy + oh);
                        match *corner {
                            0 => {
                                x0 = snapv(ox + dx);
                                y0 = snapv(oy + dy);
                            }
                            1 => {
                                x1 = snapv(ox + ow + dx);
                                y0 = snapv(oy + dy);
                            }
                            2 => {
                                x1 = snapv(ox + ow + dx);
                                y1 = snapv(oy + oh + dy);
                            }
                            _ => {
                                x0 = snapv(ox + dx);
                                y1 = snapv(oy + oh + dy);
                            }
                        }
                        if x1 - x0 < 8.0 {
                            if *corner == 0 || *corner == 3 {
                                x0 = x1 - 8.0;
                            } else {
                                x1 = x0 + 8.0;
                            }
                        }
                        if y1 - y0 < 4.0 {
                            if *corner <= 1 {
                                y0 = y1 - 4.0;
                            } else {
                                y1 = y0 + 4.0;
                            }
                        }
                        resize = Some((*id, x0, y0, x1 - x0, y1 - y0));
                    }
                    Drag::Rubber { cur, .. } => *cur = p,
                    Drag::Connect { cur, .. } => *cur = p,
                    Drag::MoveMessage { id, orig, start } => msg = Some((*id, *orig + dp.1 - *start)),
                    Drag::BarEdge { lifeline, depth, top, y0, y1, splits0, spans0 } => bar_edit = Some((*lifeline, *depth, *top, *y0, *y1, splits0.clone(), spans0.clone(), snapv(dp.1))),
                    Drag::Divider { id, idx, .. } => divider = Some((*id, *idx, snapv(dp.1))),
                    Drag::PartEdge { id, idx, vertical, .. } => part_edge = Some((*id, *idx, *vertical, (snapv(dp.0), snapv(dp.1)))),
                    Drag::None => {}
                }
                // keep the bar selected by its edge that doesn't move
                if let Some((lid, depth, top, y0, y1, ..)) = &bar_edit {
                    self.sel_bar = Some((*lid, *depth, if *top { *y1 - 1.0 } else { *y0 + 1.0 }));
                }
                if let Some(dm) = self.diag_mut() {
                    for (id, x, y) in moves {
                        if let Some(s) = dm.shape_mut(id) {
                            s.x = x;
                            s.y = y;
                        }
                        // a created lifeline takes its Create message along (the line meets its head)
                        if dm.kind == DiagramKind::Sequence {
                            if let Some(head) = dm.shape(id).filter(|s| s.kind.is_lifeline()).map(|s| s.y + lifeline_head(s.kind) / 2.0) {
                                for c in dm.connectors.iter_mut().filter(|c| c.kind == ConnKind::CreateMessage && c.to == id) {
                                    c.y = head;
                                }
                            }
                        }
                    }
                    if let Some((id, x, y, w, h)) = resize {
                        if let Some(s) = dm.shape_mut(id) {
                            s.x = x;
                            s.y = y;
                            s.w = w;
                            s.h = h;
                        }
                    }
                    if let Some((id, idx, vertical, (px, py))) = part_edge {
                        if let Some(s) = dm.shape_mut(id) {
                            let along = if vertical { px - s.x } else { py - s.y };
                            s.set_partition_edge(idx, along);
                        }
                    }
                    if let Some((id, idx, ny)) = divider {
                        // a section line stays between its neighbours (and below the fragment's tab)
                        if let Some(s) = dm.shape_mut(id) {
                            set_divider(s, idx, ny);
                        }
                    }
                    if let Some((lid, depth, top, y0, y1, splits0, spans0, ny)) = bar_edit.take() {
                        if let Some(s) = dm.shape_mut(lid) {
                            s.splits = splits0;
                            s.act_spans = spans0;
                            stretch_activation(s, depth, top, y0, y1, ny);
                        }
                    }
                    if let Some((id, y)) = msg {
                        if let Some(c) = dm.conn_mut(id) {
                            c.y = y;
                        }
                        // moving a Create message moves the head of the lifeline it creates (its bottom stays)
                        let target = dm.connectors.iter().find(|c| c.id == id && c.kind == ConnKind::CreateMessage && c.from != c.to).map(|c| c.to);
                        if let Some(s) = target.and_then(|t| dm.shape_mut(t)) {
                            if s.kind.is_lifeline() {
                                let bottom = s.y + s.h;
                                s.y = y - lifeline_head(s.kind) / 2.0;
                                s.h = (bottom - s.y).max(lifeline_head(s.kind) + 60.0);
                            }
                        }
                    }
                    for (id, pts) in line_moves {
                        if let Some(c) = dm.conn_mut(id) {
                            c.points = pts;
                        }
                    }
                    if let Some((id, f, t, pts)) = line_move {
                        // each end goes to the point of its shape's outline nearest its moved place
                        let ends = dm.connectors.iter().find(|c| c.id == id).map(|c| (c.from, c.to));
                        if let Some((fs, ts)) = ends {
                            let fa = dm.shape(fs).map(|s| render::nearest_anchor(s, f));
                            let ta = dm.shape(ts).map(|s| render::nearest_anchor(s, t));
                            if let Some(c) = dm.conn_mut(id) {
                                if fa.is_some() {
                                    c.from_anchor = fa;
                                }
                                if ta.is_some() {
                                    c.to_anchor = ta;
                                }
                                c.points = pts;
                            }
                        }
                    }
                    if let Some((id, to_end)) = end_move {
                        // slide the end along its shape's outline, to the point nearest the mouse
                        let sid = dm.connectors.iter().find(|c| c.id == id).map(|c| if to_end { c.to } else { c.from });
                        if let Some(s) = sid.and_then(|s| dm.shape(s)).cloned() {
                            let f = render::nearest_anchor(&s, dp);
                            if let Some(c) = dm.conn_mut(id) {
                                if to_end {
                                    c.to_anchor = Some(f);
                                } else {
                                    c.from_anchor = Some(f);
                                }
                            }
                        }
                    }
                    match label_move {
                        Some((LabelTarget::Conn(id), off)) => {
                            if let Some(c) = dm.conn_mut(id) {
                                c.label_offset = off;
                            }
                        }
                        Some((LabelTarget::Shape(id), off)) => {
                            if let Some(s) = dm.shape_mut(id) {
                                s.label_offset = off;
                            }
                        }
                        Some((LabelTarget::ConnFrom(id), off)) => {
                            if let Some(c) = dm.conn_mut(id) {
                                c.from_label_offset = off;
                            }
                        }
                        Some((LabelTarget::ConnTo(id), off)) => {
                            if let Some(c) = dm.conn_mut(id) {
                                c.to_label_offset = off;
                            }
                        }
                        None => {}
                    }
                    if let Some((id, idx, p)) = point {
                        if let Some(c) = dm.conn_mut(id) {
                            if let Some(slot) = c.points.get_mut(idx) {
                                *slot = p;
                            }
                        }
                    }
                }
            }
        }

        if resp.drag_stopped_by(PointerButton::Primary) {
            let drag = std::mem::replace(&mut self.drag, Drag::None);
            match drag {
                Drag::Rubber { start, cur } => {
                    let r = Rect::from_two_pos(start, cur);
                    for s in &d.shapes {
                        if r.contains_rect(cam.rect(s)) && !self.sel_shapes.contains(&s.id) {
                            self.sel_shapes.push(s.id);
                        }
                    }
                    for c in &d.connectors {
                        if self.sel_shapes.contains(&c.from) && self.sel_shapes.contains(&c.to) && !self.sel_conns.contains(&c.id) {
                            self.sel_conns.push(c.id);
                        }
                    }
                }
                Drag::Connect { from, start, cur } => {
                    if let Tool::Conn(kind) = self.tool {
                        if kind == ConnKind::Anchor {
                            if let Some(EditTarget::Shape(to) | EditTarget::Conn(to)) = self.hit_any(cam, cur) {
                                self.create_anchor(from, to);
                            }
                        } else if let (true, Some(src)) = (d.kind == DiagramKind::Sequence && matches!(kind, ConnKind::Message | ConnKind::AsyncMessage | ConnKind::CreateMessage), d.shape(from).filter(|s| s.kind.is_lifeline())) {
                            // a sequence message: to another lifeline; back onto (or near) its own
                            // lifeline = a self message; onto empty space = a new lifeline there.
                            // Fragments it is dropped inside don't count.
                            let x = cam.to_diagram(cur).0;
                            let target = self.hit_shape(d, cam, cur, true).filter(|id| d.shape(*id).map(|s| !s.kind.is_container()).unwrap_or(false));
                            let near_self = (x - (src.x + src.w / 2.0)).abs() < SELF_REACH;
                            match target {
                                Some(to) if to != from => self.create_connector(kind, from, to, start.1),
                                Some(_) if kind != ConnKind::CreateMessage => self.create_connector(kind, from, from, start.1),
                                None if near_self && kind != ConnKind::CreateMessage => self.create_connector(kind, from, from, start.1),
                                None if !near_self => self.message_to_new_lifeline(kind, from, x, start.1),
                                _ => {}
                            }
                        } else if let Some(to) = self.hit_shape(d, cam, cur, true) {
                            let before = self.diag().map(|dm| dm.connectors.len()).unwrap_or(0);
                            self.create_connector(kind, from, to, start.1);
                            // where the drag started picks the line's lane between two boxes
                            if !(d.kind == DiagramKind::Sequence && kind.is_message()) && from != to {
                                if let (Some(a), Some(b)) = (d.shape(from).cloned(), d.shape(to).cloned()) {
                                    if let Some((fa, fb)) = press_lane(&a, &b, start) {
                                        if let Some(dm) = self.diag_mut() {
                                            if dm.connectors.len() > before {
                                                if let Some(c) = dm.connectors.last_mut() {
                                                    c.from_anchor = Some(fa);
                                                    c.to_anchor = Some(fb);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Drag::PartEdge { id, orig, .. } => {
                    let now: Vec<f32> = self.diag().and_then(|dm| dm.shape(id)).map(|s| s.lane_partitions().iter().map(|p| p.size).collect()).unwrap_or_default();
                    if now == orig {
                        self.undo.pop();
                    }
                }
                Drag::Divider { id, idx, orig } => {
                    let moved = self.diag().and_then(|dm| dm.shape(id)).map(|s| s.alt_dividers().get(idx).map(|v| (v.y - orig).abs() > 0.01).unwrap_or(false)).unwrap_or(false);
                    if !moved {
                        self.undo.pop();
                    }
                }
                Drag::BarEdge { lifeline, splits0, spans0, .. } => {
                    let same = self.diag().and_then(|dm| dm.shape(lifeline)).map(|s| s.splits == splits0 && s.act_spans == spans0).unwrap_or(true);
                    if same {
                        self.undo.pop();
                    }
                }
                Drag::Move { items, .. } => {
                    // nothing moved: drop the undo step again
                    let moved = self
                        .diag()
                        .map(|dm| items.iter().any(|(id, x, y)| dm.shape(*id).map(|s| s.x != *x || s.y != *y).unwrap_or(false)))
                        .unwrap_or(false);
                    if !moved {
                        self.undo.pop();
                    } else {
                        if items.len() == 1 {
                            // a port dropped near a component attaches to its edge
                            self.snap_port_to_owner(items[0].0);
                        }
                        // a ball dropped near a socket (or a socket near a ball) joins it
                        let ids: Vec<u64> = items.iter().map(|(id, _, _)| *id).collect();
                        self.join_interfaces(&ids);
                    }
                }
                _ => {}
            }
        }

        // clicks
        if resp.clicked_by(PointerButton::Primary) {
            if let Some(p) = pointer {
                // a click on an activation bar selects the bar
                let bar_hit = self.tool == Tool::Select && !shift && self.bar_click(d, cam, p);
                match self.tool {
                    Tool::Shape(kind) => {
                        self.commit_edit();
                        let (x, y) = cam.to_diagram(p);
                        self.create_shape(kind, x, y);
                    }
                    Tool::Conn(kind) => {
                        // clicking a lifeline with Message or Asynchronous Message draws a self message there
                        if d.kind == DiagramKind::Sequence && matches!(kind, ConnKind::Message | ConnKind::AsyncMessage) {
                            let hit = self.hit_shape(d, cam, p, true).filter(|id| d.shape(*id).map(|s| s.kind.is_lifeline()).unwrap_or(false));
                            if let Some(id) = hit {
                                self.commit_edit();
                                self.create_connector(kind, id, id, cam.to_diagram(p).1);
                            }
                        }
                    }
                    Tool::Select if bar_hit => {}
                    Tool::Select => match self.label_at(p).map(|t| t.owner()).or_else(|| self.hit_any(cam, p)) {
                        Some(EditTarget::Shape(id)) => {
                            if shift {
                                if let Some(i) = self.sel_shapes.iter().position(|x| *x == id) {
                                    self.sel_shapes.remove(i);
                                } else {
                                    self.sel_shapes.push(id);
                                }
                            } else {
                                self.clear_selection();
                                self.sel_shapes.push(id);
                            }
                        }
                        Some(EditTarget::Conn(id)) => {
                            if !shift {
                                self.clear_selection();
                            }
                            self.sel_conns.push(id);
                        }
                        None => {
                            self.clear_selection();
                        }
                    },
                }
            }
        }
        if resp.double_clicked_by(PointerButton::Primary) && self.tool == Tool::Select {
            if let Some(p) = pointer {
                self.double_click(cam, d, p);
            }
        }
        if resp.secondary_clicked() {
            self.seq_menu = None;
            self.alt_menu = None;
            self.lane_menu = None;
            if let Some(p) = pointer {
                let (x, y) = cam.to_diagram(p);
                if !Self::over_plain_shape(&d, cam, p) {
                    self.lane_menu = d.shapes.iter().rev().find(|s| s.kind.is_swimlane() && x >= s.x && x <= s.x + s.w && y >= s.y && y <= s.y + s.h).map(|s| (s.id, if s.kind == ShapeKind::VerticalSwimlane { x - s.x } else { y - s.y }));
                }
                self.alt_menu = d.shapes.iter().rev().find(|s| s.kind == ShapeKind::AltFragment && x >= s.x && x <= s.x + s.w && y >= s.y && y <= s.y + s.h).map(|s| (s.id, y - s.y));
            }
            if let Some(p) = pointer {
                // a lifeline's activation bar or split point under the pointer (sequence diagrams)
                let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                self.seq_menu = seq_menu_target(&d, &opts, cam.zoom, cam.to_diagram(p));
                match self.hit_any(cam, p) {
                    Some(EditTarget::Shape(id)) => {
                        if !self.sel_shapes.contains(&id) {
                            self.clear_selection();
                            self.sel_shapes.push(id);
                        }
                    }
                    Some(EditTarget::Conn(id)) => {
                        if !self.sel_conns.contains(&id) {
                            self.clear_selection();
                            self.sel_conns.push(id);
                        }
                    }
                    None => self.clear_selection(),
                }
            }
            self.tool = Tool::Select;
        }
        self.canvas_cursor(&ctx, resp, cam, d, pointer);
        resp.context_menu(|ui| self.canvas_menu(ui));
    }

    /// The mouse pointer tells what a press would do: diagonal arrows on resize handles,
    /// a hand on movable names and bend points, a cross while drawing.
    fn canvas_cursor(&self, ctx: &egui::Context, resp: &egui::Response, cam: &Cam, d: &Diagram, pointer: Option<Pos2>) {
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

    fn canvas_menu(&mut self, ui: &mut egui::Ui) {
        // a swimlane: add a partition after the one under the pointer, or take that one away
        if let Some((id, along)) = self.lane_menu {
            let (idx, count) = self.diag().and_then(|d| d.shape(id)).map(|s| (s.partition_at(along), s.lane_partitions().len())).unwrap_or((0, 0));
            if self.menu_item(ui, "Add Partition", None, true) {
                self.add_partition(id, idx);
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
                self.add_alt_section(id);
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

    fn start_edit(&mut self, target: Option<EditTarget>) {
        let Some(t) = target else { return };
        let Some(d) = self.diag() else { return };
        let text = match t {
            EditTarget::Shape(id) => d.shape(id).map(|s| s.name.clone()),
            EditTarget::Conn(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.name.clone()),
        };
        if let Some(text) = text {
            self.editing = Some(InlineEdit { target: t, text, focus: true, line: None, new_line: false, part: None });
        }
    }

    /// Which attribute (false) or operation (true) row of a shape is under a screen point.
    /// The index is None when the point is in the list's empty space (a new row).
    fn line_at(&self, s: &Shape, cam: &Cam, p: Pos2) -> Option<(bool, Option<usize>)> {
        let opts = self.project.as_ref()?.options.clone();
        let c = render::compartments(s, &opts, &self.look())?;
        let ly = (p.y - cam.pt(0.0, s.y).y) / cam.zoom;
        let row = |y0: f32, len: usize| -> Option<usize> {
            let i = ((ly - y0 - 4.0) / c.lh).floor();
            if i >= 0.0 && (i as usize) < len {
                Some(i as usize)
            } else {
                None
            }
        };
        if let Some((a0, a1)) = c.attr {
            if ly >= a0 && ly < a1 {
                return Some((false, row(a0, s.attributes.len())));
            }
        }
        if let Some((o0, o1)) = c.ops {
            if ly >= o0 && ly <= o1 {
                return Some((true, row(o0, s.operations.len())));
            }
        }
        None
    }

    /// Edit one attribute (ops = false) or operation row in place; None adds a new row.
    fn start_line_edit(&mut self, id: u64, ops: bool, idx: Option<usize>) {
        let Some(s) = self.diag().and_then(|d| d.shape(id)).cloned() else { return };
        let list = if ops { &s.operations } else { &s.attributes };
        let (idx, text, new_line) = match idx {
            Some(i) if i < list.len() => (i, list[i].clone(), false),
            _ => {
                let n = list.len();
                self.checkpoint();
                if let Some(t) = self.diag_mut().and_then(|d| d.shape_mut(id)) {
                    if ops {
                        t.operations.push(String::new());
                    } else {
                        t.attributes.push(String::new());
                    }
                }
                (n, String::new(), true)
            }
        };
        self.clear_selection();
        self.sel_shapes.push(id);
        self.editing = Some(InlineEdit { target: EditTarget::Shape(id), text, focus: true, line: Some((ops, idx)), new_line, part: None });
    }

    fn double_click(&mut self, cam: &Cam, d: &Diagram, p: Pos2) {
        // an end of the selected line: let it be placed automatically again
        if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
            if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                if let Some(ends) = self.end_handles(d, cam, c) {
                    if let Some(k) = ends.iter().position(|e| (*e - p).length() < 8.0) {
                        let cid = c.id;
                        self.checkpoint();
                        if let Some(cm) = self.diag_mut().and_then(|dm| dm.conn_mut(cid)) {
                            if k == 0 {
                                cm.from_anchor = None;
                            } else {
                                cm.to_anchor = None;
                            }
                        }
                        return;
                    }
                }
            }
        }
        // a bend point of the selected line: remove it
        if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
            if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                if let Some(PointHit::Point(i)) = Self::point_handle_hit(d, cam, c, p) {
                    let cid = c.id;
                    self.checkpoint();
                    if let Some(cm) = self.diag_mut().and_then(|dm| dm.conn_mut(cid)) {
                        if i < cm.points.len() {
                            cm.points.remove(i);
                        }
                    }
                    return;
                }
            }
        }
        // a partition's header: rename that partition
        if self.label_at(p).is_none() && !Self::over_plain_shape(d, cam, p) {
            let (x, y) = cam.to_diagram(p);
            for s in d.shapes.iter().rev().filter(|s| s.kind.is_swimlane()) {
                let parts = s.lane_partitions();
                if let Some(i) = (0..parts.len()).find(|&i| s.partition_header(i).map(|(hx, hy, hw, hh)| x >= hx && x <= hx + hw && y >= hy && y <= hy + hh).unwrap_or(false)) {
                    self.clear_selection();
                    self.sel_shapes.push(s.id);
                    self.editing = Some(InlineEdit { target: EditTarget::Shape(s.id), text: parts[i].name.clone(), focus: true, line: None, new_line: false, part: Some(i) });
                    return;
                }
            }
        }
        let target = self.label_at(p).map(|t| t.owner()).or_else(|| self.hit_any(cam, p));
        if let Some(EditTarget::Shape(id)) = target {
            if let Some(s) = d.shape(id) {
                if let Some((ops, idx)) = self.line_at(s, cam, p) {
                    self.start_line_edit(id, ops, idx);
                    return;
                }
            }
        }
        self.start_edit(target);
    }

    /// Apply the inline editor's text, if one is open.
    fn commit_edit(&mut self) {
        let Some(ed) = self.editing.take() else { return };
        if let (EditTarget::Shape(id), Some((ops, idx))) = (ed.target, ed.line) {
            let text = ed.text.trim().to_string();
            let current = self.diag().and_then(|d| d.shape(id)).and_then(|s| if ops { s.operations.get(idx) } else { s.attributes.get(idx) }).cloned();
            let Some(current) = current else { return };
            if current == text && !text.is_empty() {
                return;
            }
            if !ed.new_line {
                self.checkpoint();
            }
            if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) {
                let list = if ops { &mut s.operations } else { &mut s.attributes };
                if text.is_empty() {
                    if idx < list.len() {
                        list.remove(idx);
                    }
                } else if idx < list.len() {
                    list[idx] = text.clone();
                }
            }
            if ed.new_line && text.is_empty() {
                self.undo.pop();
            }
            return;
        }
        if let (EditTarget::Shape(id), Some(i)) = (ed.target, ed.part) {
            let current = self.diag().and_then(|d| d.shape(id)).and_then(|s| s.lane_partitions().get(i).map(|p| p.name.clone()));
            if current.is_none() || current.as_deref() == Some(ed.text.as_str()) {
                return;
            }
            self.checkpoint();
            if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) {
                s.normalize_partitions();
                if let Some(p) = s.partitions.as_mut().and_then(|v| v.get_mut(i)) {
                    p.name = ed.text;
                }
            }
            return;
        }
        let Some(d) = self.diag() else { return };
        let current = match ed.target {
            EditTarget::Shape(id) => d.shape(id).map(|s| s.name.clone()),
            EditTarget::Conn(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.name.clone()),
        };
        if current.is_none() || current.as_deref() == Some(ed.text.as_str()) {
            return;
        }
        self.checkpoint();
        if let Some(dm) = self.diag_mut() {
            match ed.target {
                EditTarget::Shape(id) => {
                    if let Some(s) = dm.shape_mut(id) {
                        s.name = ed.text;
                    }
                }
                EditTarget::Conn(id) => {
                    if let Some(c) = dm.conn_mut(id) {
                        c.name = ed.text;
                    }
                }
            }
        }
    }

    /// Close the inline editor without applying it (a row added for it is removed).
    fn cancel_edit(&mut self) {
        let Some(ed) = self.editing.take() else { return };
        if let (EditTarget::Shape(id), Some((ops, idx)), true) = (ed.target, ed.line, ed.new_line) {
            if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) {
                let list = if ops { &mut s.operations } else { &mut s.attributes };
                if idx < list.len() && list[idx].is_empty() {
                    list.remove(idx);
                }
            }
            self.undo.pop();
        }
    }

    fn inline_editor(&mut self, ctx: &egui::Context, cam: &Cam) {
        let Some(ed) = self.editing.as_ref() else { return };
        let target = ed.target;
        let line = ed.line;
        let part = ed.part;
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let look = self.look();
        let ds = opts.connector_style;
        let Some(d) = self.diag() else {
            self.editing = None;
            return;
        };
        // (position, width, multiline, left aligned)
        let part_place = match (target, part) {
            (EditTarget::Shape(id), Some(i)) => Some(d.shape(id).and_then(|s| {
                let (x, y, w, _) = s.partition_header(i)?;
                let a = cam.pt(x, y);
                let sw = w * cam.zoom;
                let ww = sw.clamp(100.0, 300.0);
                Some((Pos2::new(a.x + sw / 2.0 - ww / 2.0, a.y + 2.0), ww, false, false))
            })),
            _ => None,
        };
        let placed = if let Some(p) = part_place { p } else { match (target, line) {
            (EditTarget::Shape(id), Some((ops, idx))) => d.shape(id).and_then(|s| {
                let c = render::compartments(s, &opts, &look)?;
                let (y0, _) = if ops { c.ops? } else { c.attr? };
                let r = cam.rect(s);
                let y = cam.pt(0.0, s.y + y0 + 4.0 + idx as f32 * c.lh).y - 3.0;
                Some((Pos2::new(r.left() + 2.0, y), (r.width() - 4.0).max(160.0), false, true))
            }),
            (EditTarget::Shape(id), None) => d.shape(id).map(|s| {
                let r = cam.rect(s);
                let w = r.width().clamp(120.0, 320.0);
                let y = if s.kind.is_pseudo() { r.bottom() + 2.0 } else { r.top() + 4.0 };
                if s.kind == ShapeKind::Text {
                    // free text is edited in place, over the text itself
                    return (r.left_top(), r.width().max(160.0), true, true);
                }
                (Pos2::new(r.center().x - w / 2.0, y), w, s.kind == ShapeKind::Note, false)
            }),
            (EditTarget::Conn(id), _) => d.connectors.iter().find(|c| c.id == id).and_then(|c| {
                let (a, b) = (d.shape(c.from)?, d.shape(c.to)?);
                let path = render::conn_path(d.kind, c, a, b, ds);
                let k = path.len() / 2;
                let (p0, p1) = (path[k.saturating_sub(1)], path[k]);
                let mid = cam.pt((p0.0 + p1.0) / 2.0, (p0.1 + p1.1) / 2.0);
                Some((mid - Vec2::new(80.0, 26.0), 160.0, false, false))
            }),
        } };
        let Some((pos, width, multiline, left)) = placed else {
            self.editing = None;
            return;
        };
        let hint = match line {
            Some((false, _)) => "-name : Type",
            Some((true, _)) => "+operation() : void",
            None => "",
        };
        let mut commit = false;
        let mut cancel = false;
        let ed = self.editing.as_mut().unwrap();
        egui::Area::new(egui::Id::new("inline_edit")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
            let te = if multiline {
                egui::TextEdit::multiline(&mut ed.text).desired_width(width).desired_rows(3)
            } else {
                let align = if left { egui::Align::Min } else { egui::Align::Center };
                egui::TextEdit::singleline(&mut ed.text).desired_width(width).horizontal_align(align).hint_text(hint)
            };
            let r = ui.add(te);
            if ed.focus {
                r.request_focus();
                ed.focus = false;
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                cancel = true;
            } else if r.lost_focus() {
                commit = true;
            }
        });
        if cancel {
            self.cancel_edit();
        } else if commit {
            self.commit_edit();
        }
    }

    /// Screen positions of the two ends of a line between shapes (not sequence messages).
    fn end_handles(&self, d: &Diagram, cam: &Cam, c: &Connector) -> Option<[Pos2; 2]> {
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
    fn point_handles(d: &Diagram, cam: &Cam, c: &Connector) -> Option<(Vec<Pos2>, Vec<Pos2>)> {
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

    fn point_handle_hit(d: &Diagram, cam: &Cam, c: &Connector, p: Pos2) -> Option<PointHit> {
        let (points, mids) = Self::point_handles(d, cam, c)?;
        if let Some(i) = points.iter().position(|q| (*q - p).length() < 7.0) {
            return Some(PointHit::Point(i));
        }
        mids.iter().position(|q| (*q - p).length() < 7.0).map(PointHit::Mid)
    }

    /// Show or hide the arrow at one end by changing the ends' navigability.
    /// The draggable name label under a screen point (the topmost one).
    fn label_at(&self, p: Pos2) -> Option<LabelTarget> {
        self.label_rects.iter().rev().find(|(_, r)| r.contains(p)).map(|(t, _)| *t)
    }

    fn set_arrows(&mut self, id: u64, end: Option<bool>, start: Option<bool>) {
        use crate::model::{NAV_TRUE, NAV_UNSPECIFIED};
        self.checkpoint();
        if let Some(c) = self.diag_mut().and_then(|d| d.conn_mut(id)) {
            c.arrow_end = false;
            c.arrow_start = false;
            let set = |me: &mut u8, other: &mut u8, on: bool| {
                if on {
                    *me = NAV_TRUE;
                    if *other == NAV_TRUE {
                        *other = NAV_UNSPECIFIED;
                    }
                } else if *me == NAV_TRUE {
                    *me = NAV_UNSPECIFIED;
                }
            };
            if let Some(e) = end {
                set(&mut c.nav_to, &mut c.nav_from, e);
            }
            if let Some(s) = start {
                set(&mut c.nav_from, &mut c.nav_to, s);
            }
        }
    }

    fn set_conn_style(&mut self, id: u64, style: u8) {
        self.checkpoint();
        if let Some(c) = self.diag_mut().and_then(|d| d.conn_mut(id)) {
            c.style = style;
        }
    }

    #[allow(dead_code)]
    fn default_style(&self) -> u8 {
        self.project.as_ref().map(|p| p.options.connector_style).unwrap_or(render::STYLE_OBLIQUE)
    }

    /// Right-angle: the Rectilinear style (code 0). Straight: no bend points, Oblique (code 1).
    fn set_line_shape(&mut self, id: u64, right_angle: bool) {
        self.checkpoint();
        if let Some(c) = self.diag_mut().and_then(|d| d.conn_mut(id)) {
            if right_angle {
                c.style = render::STYLE_RECTILINEAR;
            } else {
                c.points.clear();
                c.style = render::STYLE_OBLIQUE;
            }
        }
    }

    #[allow(dead_code)]
    fn set_line_elbow(&mut self, id: u64, right_angle: bool) {
        let Some(d) = self.diag() else { return };
        let Some(c) = d.connectors.iter().find(|c| c.id == id) else { return };
        let (Some(a), Some(b)) = (d.shape(c.from), d.shape(c.to)) else { return };
        let ((ax, ay), (bx, by)) = (a.center(), b.center());
        let points = if !right_angle {
            vec![]
        } else if c.from == c.to {
            let (r, t) = (a.x + a.w + 30.0, a.y - 30.0);
            vec![(ax, t), (r, t), (r, ay)]
        } else if (bx - ax).abs() >= (by - ay).abs() {
            let mx = (ax + bx) / 2.0;
            vec![(mx, ay), (mx, by)]
        } else {
            let my = (ay + by) / 2.0;
            vec![(ax, my), (bx, my)]
        };
        self.checkpoint();
        if let Some(cm) = self.diag_mut().and_then(|d| d.conn_mut(id)) {
            cm.points = points;
        }
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

    // ---------- shortcuts ----------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.menu_keys(ctx) {
            return;
        }
        let typing = ctx.wants_keyboard_input();
        let dialog_open = self.switcher.is_some() || self.export_dlg.is_some() || self.batch.is_some() || self.print_dlg.is_some() || self.printing.is_some() || self.confirm.is_some() || self.confirm_delete.is_some() || self.new_diagram.is_some() || self.app_opts.is_some() || self.proj_opts.is_some() || self.rename.is_some();
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
        if typing || dialog_open {
            return;
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

    // ---------- dialogs ----------

    fn dialogs(&mut self, ctx: &egui::Context) {
        self.new_diagram_dialog(ctx);
        self.app_options_dialog(ctx);
        self.project_options_dialog(ctx);
        self.switcher_dialog(ctx);
        self.rename_dialog(ctx);
        self.export_dialog(ctx);
        self.print_dialog(ctx);
        self.job_progress(ctx);
        self.update_dialogs(ctx);
        self.confirm_dialogs(ctx);
    }

    fn new_diagram_dialog(&mut self, ctx: &egui::Context) {
        let Some((mut kind, mut name)) = self.new_diagram.clone() else { return };
        let lang = self.settings.language;
        let look = self.look();
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let hint = self.project.as_ref().map(|p| p.next_diagram_name(kind)).unwrap_or_else(|| format!("{}1", kind.label()));
        let folder_list = self.project.as_ref().map(folder_paths).unwrap_or_default();
        let mut folder_sel = self.current_folder.filter(|f| folder_list.iter().any(|(id, _)| id == f));
        let mut open = true;
        let mut create = false;
        let mut cancel = false;
        let esc = esc_cancels(ctx);
        egui::Window::new(tr(lang, "New Diagram...").trim_end_matches("..."))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                modal_dialog(ui);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(230.0);
                        ui.label(egui::RichText::new(tr(lang, "Diagram type")).strong());
                        for k in DiagramKind::ALL {
                            let r = ui.selectable_label(kind == k, tr(lang, k.label()));
                            if r.clicked() {
                                kind = k;
                            }
                            if r.double_clicked() {
                                kind = k;
                                create = true;
                            }
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        ui.label(egui::RichText::new(tr(lang, kind.label())).heading());
                        ui.label(kind.description());
                        ui.add_space(8.0);
                        // a tiny preview of the toolbox
                        ui.horizontal_wrapped(|ui| {
                            for e in palette::palette(kind).into_iter().take(14) {
                                let (r, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                                render::draw_icon(ui.painter(), r, e.tool, &look, &opts);
                                resp.on_hover_text(e.label);
                            }
                        });
                        ui.add_space(10.0);
                        ui.label(tr(lang, "Folder"));
                        let shown = folder_sel.and_then(|f| folder_list.iter().find(|(id, _)| *id == f)).map(|(_, p)| p.clone()).unwrap_or_else(|| tr(lang, "(Project root)").to_string());
                        egui::ComboBox::from_id_salt("new_diag_folder").width(280.0).selected_text(shown).show_ui(ui, |ui| {
                            ui.selectable_value(&mut folder_sel, None, tr(lang, "(Project root)"));
                            for (id, path) in &folder_list {
                                ui.selectable_value(&mut folder_sel, Some(*id), path);
                            }
                        });
                        ui.add_space(6.0);
                        ui.label(tr(lang, "Diagram name"));
                        let r = ui.add(egui::TextEdit::singleline(&mut name).hint_text(hint.clone()).desired_width(280.0));
                        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            create = true;
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "OK")).clicked() {
                        create = true;
                    }
                    if ui.button(tr(lang, "Cancel")).clicked() {
                        cancel = true;
                    }
                });
            });
        if esc && !create {
            cancel = true;
        }
        self.current_folder = folder_sel;
        if create {
            self.new_diagram = None;
            let n = if name.trim().is_empty() { None } else { Some(name.trim().to_string()) };
            self.add_diagram(kind, n);
        } else if cancel || !open {
            self.new_diagram = None;
        } else {
            self.new_diagram = Some((kind, name));
        }
    }

    fn app_options_dialog(&mut self, ctx: &egui::Context) {
        let lang = self.settings.language;
        let Some(dlg) = self.app_opts.as_mut() else { return };
        let t = |s: &'static str| tr(lang, s);
        let mut open = true;
        let mut ok = false;
        let mut apply = false;
        let mut cancel = false;
        let esc = esc_cancels(ctx);
        egui::Window::new(t("Application Options"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                modal_dialog(ui);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(130.0);
                        for (i, p) in ["General", "Diagramming", "Appearance"].iter().enumerate() {
                            ui.selectable_value(&mut dlg.page, i, t(p));
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(340.0);
                        ui.set_min_height(220.0);
                        let s = &mut dlg.s;
                        match dlg.page {
                            0 => {
                                egui::Grid::new("app_general").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                                    ui.label(t("Language"));
                                    egui::ComboBox::from_id_salt("lang")
                                        .selected_text(match s.language {
                                            Language::System => t("OS Default"),
                                            Language::English => "English",
                                            Language::Korean => "한국어",
                                        })
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut s.language, Language::System, t("OS Default"));
                                            ui.selectable_value(&mut s.language, Language::English, "English");
                                            ui.selectable_value(&mut s.language, Language::Korean, "한국어");
                                        });
                                    ui.end_row();
                                    ui.label(t("Recent projects to remember"));
                                    ui.add(egui::DragValue::new(&mut s.recent_max).range(1..=20));
                                    ui.end_row();
                                });
                                ui.checkbox(&mut s.confirm_delete, t("Confirm before deleting diagrams"));
                                ui.checkbox(&mut s.check_updates, t("Check for updates at startup"));
                            }
                            1 => {
                                egui::Grid::new("app_diag").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                                    ui.label(t("Grid size"));
                                    ui.add(egui::DragValue::new(&mut s.grid_size).range(4.0..=50.0).suffix(" px"));
                                    ui.end_row();
                                    ui.label(t("Shape text size"));
                                    ui.add(egui::DragValue::new(&mut s.text_size).range(8.0..=24.0).speed(0.2));
                                    ui.end_row();
                                });
                                ui.checkbox(&mut s.show_grid, t("Show grid in new diagrams"));
                                ui.checkbox(&mut s.snap, t("Snap shapes to grid"));
                                ui.checkbox(&mut s.palette_labels, t("Show labels in diagram toolbar"));
                            }
                            _ => {
                                egui::Grid::new("app_look").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                                    ui.label(t("Theme"));
                                    ui.horizontal(|ui| {
                                        ui.radio_value(&mut s.dark, false, t("Light"));
                                        ui.radio_value(&mut s.dark, true, t("Dark"));
                                    });
                                    ui.end_row();
                                    ui.label(t("Diagram font"));
                                    let shown = if s.font.is_empty() { t("(Default)").to_string() } else { s.font.clone() };
                                    egui::ComboBox::from_id_salt("diagram_font").selected_text(shown).show_ui(ui, |ui| {
                                        if ui.selectable_label(s.font.is_empty(), t("(Default)")).clicked() {
                                            s.font.clear();
                                        }
                                        for f in crate::fonts::available() {
                                            if ui.selectable_label(s.font == *f, *f).clicked() {
                                                s.font = f.to_string();
                                            }
                                        }
                                    });
                                    ui.end_row();
                                    ui.label(t("Shape text size"));
                                    ui.add(egui::DragValue::new(&mut s.text_size).range(8.0..=24.0).speed(0.2));
                                    ui.end_row();
                                });
                            }
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(t("OK")).clicked() {
                        ok = true;
                    }
                    if ui.button(t("Cancel")).clicked() {
                        cancel = true;
                    }
                    if ui.button(t("Apply")).clicked() {
                        apply = true;
                    }
                });
            });
        if ok || apply {
            let mut s = self.app_opts.as_ref().unwrap().s.clone();
            s.recent = self.settings.recent.clone();
            s.recent.truncate(s.recent_max.max(1));
            s.skip_version = self.settings.skip_version.clone();
            self.settings = s;
            self.settings.save();
            self.apply_theme(ctx);
            self.fonts_check = true;
        }
        if esc && !ok {
            cancel = true;
        }
        if ok || cancel || !open {
            self.app_opts = None;
        }
    }

    fn project_options_dialog(&mut self, ctx: &egui::Context) {
        let lang = self.settings.language;
        let Some(dlg) = self.proj_opts.as_mut() else { return };
        let t = |s: &'static str| tr(lang, s);
        let mut open = true;
        let mut ok = false;
        let mut apply = false;
        let mut cancel = false;
        let esc = esc_cancels(ctx);
        egui::Window::new(t("Project Options"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                modal_dialog(ui);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(150.0);
                        for (i, p) in ["General", "Diagramming", "Class", "Sequence", "Entity Relationship Diagram"].iter().enumerate() {
                            ui.selectable_value(&mut dlg.page, i, t(p));
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(360.0);
                        ui.set_min_height(220.0);
                        let o = &mut dlg.o;
                        match dlg.page {
                            0 => {
                                egui::Grid::new("proj_general").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                                    ui.label(t("Name"));
                                    ui.add(egui::TextEdit::singleline(&mut dlg.name).desired_width(240.0));
                                    ui.end_row();
                                    ui.label(t("Author"));
                                    ui.add(egui::TextEdit::singleline(&mut dlg.author).desired_width(240.0));
                                    ui.end_row();
                                });
                                ui.label(t("Description"));
                                ui.add(egui::TextEdit::multiline(&mut dlg.description).desired_rows(5).desired_width(340.0));
                            }
                            1 => {
                                egui::Grid::new("proj_look").num_columns(3).spacing([10.0, 6.0]).show(ui, |ui| {
                                    let defaults = ProjectOptions::default();
                                    ui.label(t("Diagram background"));
                                    ui.color_edit_button_srgb(&mut o.diagram_background);
                                    if ui.small_button(t("Default")).clicked() {
                                        o.diagram_background = defaults.diagram_background;
                                    }
                                    ui.end_row();
                                    ui.label(t("Shape fill"));
                                    ui.color_edit_button_srgb(&mut o.shape_fill);
                                    if ui.small_button(t("Default")).clicked() {
                                        o.shape_fill = defaults.shape_fill;
                                    }
                                    ui.end_row();
                                    ui.label(t("Text color"));
                                    ui.color_edit_button_srgb(&mut o.text_color);
                                    if ui.small_button(t("Default")).clicked() {
                                        o.text_color = defaults.text_color;
                                    }
                                    ui.end_row();
                                    ui.label(t("Font"));
                                    let shown = if o.font.is_empty() { t("(Default)").to_string() } else { o.font.clone() };
                                    egui::ComboBox::from_id_salt("proj_font").selected_text(shown).show_ui(ui, |ui| {
                                        if ui.selectable_label(o.font.is_empty(), t("(Default)")).clicked() {
                                            o.font.clear();
                                        }
                                        for f in crate::fonts::available() {
                                            if ui.selectable_label(o.font == *f, *f).clicked() {
                                                o.font = f.to_string();
                                            }
                                        }
                                    });
                                    if ui.small_button(t("Use original blue")).clicked() {
                                        o.shape_fill = [122, 207, 245];
                                    }
                                    ui.end_row();
                                });
                                ui.separator();
                                ui.checkbox(&mut o.show_stereotypes, t("Show stereotypes"));
                                ui.checkbox(&mut o.show_multiplicity, t("Show multiplicity"));
                                ui.checkbox(&mut o.auto_fit_shapes, t("Grow shapes to fit their text"));
                                ui.horizontal(|ui| {
                                    ui.label(t("Default connector style"));
                                    let shown = render::LINE_STYLES.iter().find(|(v, _)| *v == o.connector_style).map(|(_, n)| *n).unwrap_or("Oblique");
                                    egui::ComboBox::from_id_salt("proj_conn_style").selected_text(t(shown)).show_ui(ui, |ui| {
                                        for (v, n) in render::LINE_STYLES.iter().skip(1) {
                                            ui.selectable_value(&mut o.connector_style, *v, t(n));
                                        }
                                    });
                                });
                            }
                            2 => {
                                ui.checkbox(&mut o.show_attributes, t("Show attributes"));
                                ui.checkbox(&mut o.show_operations, t("Show operations"));
                                ui.label(t("Default attribute visibility"));
                                ui.horizontal(|ui| {
                                    for (c, l) in [('+', "+ public"), ('-', "- private"), ('#', "# protected"), ('~', "~ package")] {
                                        ui.radio_value(&mut o.default_visibility, c, l);
                                    }
                                });
                            }
                            3 => {
                                ui.checkbox(&mut o.number_messages, t("Number sequence messages"));
                                ui.label(t("Sequence number format"));
                                ui.radio_value(&mut o.sequence_numbering, crate::model::SEQ_NESTED_LEVEL, t("Nested level (1, 1.1, 1.2, 1.2.1)"));
                                ui.radio_value(&mut o.sequence_numbering, crate::model::SEQ_SINGLE_LEVEL, t("Single level (1, 2, 3)"));
                                ui.horizontal(|ui| {
                                    ui.label(t("Sequence number handling"));
                                    ui.radio_value(&mut o.manual_numbering, false, t("Automatic"));
                                    ui.radio_value(&mut o.manual_numbering, true, t("Manual"));
                                });
                                ui.add_space(6.0);
                                ui.checkbox(&mut o.show_activations, t("Show activations"));
                                ui.checkbox(&mut o.auto_extend_activations, t("Auto extend activations"));
                            }
                            _ => {
                                ui.label(t("ERD notation"));
                                ui.radio_value(&mut o.erd_notation, ErdNotation::CrowsFoot, t("Crow's Foot"));
                                ui.radio_value(&mut o.erd_notation, ErdNotation::Multiplicity, t("Multiplicity (1, *)"));
                            }
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(t("OK")).clicked() {
                        ok = true;
                    }
                    if ui.button(t("Cancel")).clicked() {
                        cancel = true;
                    }
                    if ui.button(t("Apply")).clicked() {
                        apply = true;
                    }
                });
            });
        if ok || apply {
            let dlg = self.proj_opts.as_ref().unwrap();
            let (o, name, author, description) = (dlg.o.clone(), dlg.name.clone(), dlg.author.clone(), dlg.description.clone());
            if let Some(p) = self.project.as_mut() {
                p.options = o;
                if !name.trim().is_empty() {
                    p.name = name.trim().to_string();
                }
                p.author = author;
                p.description = description;
                self.dirty = true;
                self.fonts_check = true;
            }
        }
        if esc && !ok {
            cancel = true;
        }
        if ok || cancel || !open {
            self.proj_opts = None;
        }
    }

    fn switcher_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut idx) = self.switcher else { return };
        let items = self.switcher_items();
        if items.is_empty() {
            self.switcher = None;
            return;
        }
        let lang = self.settings.language;
        idx = idx.min(items.len() - 1);
        let mut choose: Option<u64> = None;
        let mut close = false;
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::Tab) || i.consume_key(Modifiers::SHIFT, Key::Tab) || i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                idx = (idx + items.len() - 1) % items.len();
            }
            if i.consume_key(Modifiers::CTRL, Key::Tab) || i.consume_key(Modifiers::NONE, Key::Tab) || i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                idx = (idx + 1) % items.len();
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                choose = Some(items[idx]);
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                close = true;
            }
        });
        let open_count = self.tabs.len();
        egui::Window::new(tr(lang, "Window Switcher"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                modal_dialog(ui);
                ui.set_min_width(380.0);
                ui.weak(tr(lang, "Use Up/Down or Tab, then Enter."));
                ui.add_space(4.0);
                for (i, id) in items.iter().enumerate() {
                    if i == 0 && open_count > 0 {
                        ui.label(egui::RichText::new(tr(lang, "Open diagrams")).strong());
                    }
                    if i == open_count {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(tr(lang, "All diagrams")).strong());
                    }
                    let Some(d) = self.project.as_ref().and_then(|p| p.diagram(*id)) else { continue };
                    let text = format!("{}   —   {}", d.name, tr(lang, d.kind.label()));
                    if ui.selectable_label(i == idx, text).clicked() {
                        choose = Some(*id);
                    }
                }
                ui.add_space(6.0);
                if ui.button(tr(lang, "Close")).clicked() {
                    close = true;
                }
            });
        if let Some(id) = choose {
            self.open_diagram(id);
            self.switcher = None;
        } else if close {
            self.switcher = None;
        } else {
            self.switcher = Some(idx);
        }
    }

    fn rename_dialog(&mut self, ctx: &egui::Context) {
        let Some((id, mut name)) = self.rename.clone() else { return };
        let lang = self.settings.language;
        let mut done = false;
        let mut cancel = false;
        // the first frame of this dialog puts the cursor in the box with the name selected
        let first = self.rename_focused != Some(id);
        egui::Window::new(tr(lang, "Rename"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                modal_dialog(ui);
                let out = egui::TextEdit::singleline(&mut name).desired_width(280.0).show(ui);
                let r = out.response.clone();
                if first {
                    r.request_focus();
                    let mut state = out.state;
                    let n = name.chars().count();
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
                    state.store(ui.ctx(), r.id);
                }
                // Enter works like OK and Esc like Cancel
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel = true;
                } else if (r.has_focus() || r.lost_focus()) && ui.input(|i| i.key_pressed(Key::Enter)) {
                    done = true;
                }
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "OK")).clicked() {
                        done = true;
                    }
                    if ui.button(tr(lang, "Cancel")).clicked() {
                        cancel = true;
                    }
                });
            });
        self.rename_focused = if done || cancel { None } else { Some(id) };
        if done {
            if !name.trim().is_empty() {
                if let Some(p) = self.project.as_mut() {
                    if let Some(f) = p.folders.iter_mut().find(|f| f.id == id) {
                        f.name = name.trim().to_string();
                        self.dirty = true;
                    } else if let Some(d) = p.diagram_mut(id) {
                        d.name = name.trim().to_string();
                        self.dirty = true;
                    }
                }
            }
            self.rename = None;
        } else if cancel {
            self.rename = None;
        } else {
            self.rename = Some((id, name));
        }
    }

    fn confirm_dialogs(&mut self, ctx: &egui::Context) {
        let lang = self.settings.language;
        if let Some(action) = self.confirm.clone() {
            let mut choice: Option<u8> = None;
            egui::Window::new("NapkinUML").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                modal_dialog(ui);
                ui.label(tr(lang, "Save changes to the project?"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "Yes")).clicked() {
                        choice = Some(0);
                    }
                    if ui.button(tr(lang, "No")).clicked() {
                        choice = Some(1);
                    }
                    if ui.button(tr(lang, "Cancel")).clicked() {
                        choice = Some(2);
                    }
                });
            });
            // Esc works like Cancel: nothing is saved or discarded
            if choice.is_none() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                choice = Some(2);
            }
            match choice {
                Some(0) => {
                    self.confirm = None;
                    if self.save() {
                        self.perform(action);
                    }
                }
                Some(1) => {
                    self.confirm = None;
                    self.dirty = false;
                    self.perform(action);
                }
                Some(_) => self.confirm = None,
                None => {}
            }
        }
        if let Some(id) = self.confirm_delete {
            let name = self.project.as_ref().and_then(|p| p.diagram(id)).map(|d| d.name.clone()).unwrap_or_default();
            let mut choice: Option<bool> = None;
            egui::Window::new(tr(lang, "Delete Diagram")).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                modal_dialog(ui);
                ui.label(format!("{}\n{}", tr(lang, "Delete this diagram?"), name));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "Yes")).clicked() {
                        choice = Some(true);
                    }
                    if ui.button(tr(lang, "No")).clicked() {
                        choice = Some(false);
                    }
                });
            });
            // Esc works like No: the diagram is kept
            if choice.is_none() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                choice = Some(false);
            }
            if let Some(c) = choice {
                self.confirm_delete = None;
                if c {
                    self.delete_diagram(id);
                }
            }
        }
    }

    // ---------- screenshots ----------

    fn handle_screenshots(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            if let egui::Event::Screenshot { image, .. } = e {
                match self.shot.take() {
                    Some(Shot::Check(path)) => {
                        save_png(&image, &path);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    None => {}
                }
            }
        }
        // the --screenshot option: draw about 120 frames, then take the picture
        // for checking multi-diagram export: NAPKINUML_EXPORT_ALL=<folder> exports every diagram
        // there a few frames into a --screenshot run
        if let Some((_, 10)) = self.screenshot.as_ref() {
            // for checking Copy as Image: NAPKINUML_COPY_IMAGE=1 copies (the selection, or the diagram)
            if std::env::var("NAPKINUML_COPY_IMAGE").is_ok() {
                self.copy_as_image();
            }
            // for checking Split Activation: NAPKINUML_SPLIT_AT=<x>,<y> (diagram units) right-clicks there
            // and picks Split Activation (or Join Activations when on a split point)
            // for checking activation handles: NAPKINUML_BAR_EDGE=<x>,<y>[,top|bottom,<new y>] selects the
            // bar there and (optionally) moves its top or bottom edge, as dragging its handle does
            if let Ok(v) = std::env::var("NAPKINUML_BAR_EDGE") {
                let parts: Vec<&str> = v.split(',').collect();
                let num = |i: usize| parts.get(i).and_then(|p| p.parse::<f32>().ok());
                let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                if let (Some(d), Some(x), Some(y)) = (self.diag().cloned(), num(0), num(1)) {
                    if let Some(b) = bar_at(&d, &opts, 1.0, (x, y)) {
                        self.clear_selection();
                        self.sel_bar = Some((b.lifeline, b.depth, (b.y0 + b.y1) / 2.0));
                        if let Some(ny) = num(3) {
                            let top = parts.get(2) == Some(&"top");
                            self.checkpoint();
                            if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(b.lifeline)) {
                                stretch_activation(s, b.depth, top, b.y0, b.y1, ny);
                            }
                            self.sel_bar = Some((b.lifeline, b.depth, if top { b.y1 - 1.0 } else { b.y0 + 1.0 }));
                        }
                    }
                }
            }
            // for checking alt sections: NAPKINUML_ALT=<cmd>;<cmd>... on the first alt fragment, each
            // cmd add, remove[@<y>] or move:<line>:<y> (y in diagram units, as the drag and menu do)
            if let Ok(v) = std::env::var("NAPKINUML_ALT") {
                let id = self.diag().and_then(|d| d.shapes.iter().find(|s| s.kind == ShapeKind::AltFragment).map(|s| (s.id, s.y)));
                if let Some((id, top)) = id {
                    for cmd in v.split(';') {
                        let parts: Vec<&str> = cmd.split(':').collect();
                        if cmd == "add" {
                            self.add_alt_section(id);
                        } else if let Some(rest) = cmd.strip_prefix("remove") {
                            let at = rest.strip_prefix('@').and_then(|y| y.parse::<f32>().ok()).map(|y| y - top);
                            self.remove_alt_section(id, at);
                        } else if let (Some(&"move"), Some(i), Some(y)) = (parts.first(), parts.get(1).and_then(|p| p.parse::<usize>().ok()), parts.get(2).and_then(|p| p.parse::<f32>().ok())) {
                            self.checkpoint();
                            if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
                                set_divider(s, i, y);
                            }
                        }
                    }
                    self.clear_selection();
                    self.sel_shapes.push(id);
                }
            }
            // for checking swimlanes: NAPKINUML_LANE=<cmd>;... on the first swimlane, each cmd
            // add@<along>, remove@<along>, edge:<i>:<along> or rename:<i>:<text> (along = distance
            // along the swimlane in diagram units, as the menu and the drag use)
            if let Ok(v) = std::env::var("NAPKINUML_LANE") {
                let lane = self.diag().and_then(|d| d.shapes.iter().find(|s| s.kind.is_swimlane()).map(|s| s.id));
                if let Some(id) = lane {
                    for cmd in v.split(';') {
                        let idx_at = |along: &str| self.diag().and_then(|d| d.shape(id)).map(|s| s.partition_at(along.parse().unwrap_or(0.0))).unwrap_or(0);
                        if let Some(a) = cmd.strip_prefix("add@") {
                            let i = idx_at(a);
                            self.add_partition(id, i);
                        } else if let Some(a) = cmd.strip_prefix("remove@") {
                            let i = idx_at(a);
                            self.remove_partition(id, i);
                        } else {
                            let parts: Vec<&str> = cmd.splitn(3, ':').collect();
                            match parts.as_slice() {
                                ["edge", i, a] => {
                                    self.checkpoint();
                                    if let (Ok(i), Ok(a), Some(s)) = (i.parse::<usize>(), a.parse::<f32>(), self.diag_mut().and_then(|dm| dm.shape_mut(id))) {
                                        s.set_partition_edge(i, a);
                                    }
                                }
                                ["rename", i, t] => {
                                    if let Ok(i) = i.parse::<usize>() {
                                        self.editing = Some(InlineEdit { target: EditTarget::Shape(id), text: t.to_string(), focus: false, line: None, new_line: false, part: Some(i) });
                                        self.commit_edit();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    self.clear_selection();
                    self.sel_shapes.push(id);
                }
            }
            // for checking end labels: NAPKINUML_END_LABELS=<line index>,<source>,<target>,<sdx>,<sdy>,<tdx>,<tdy>
            if let Ok(v) = std::env::var("NAPKINUML_END_LABELS") {
                let p: Vec<&str> = v.split(',').collect();
                let num = |i: usize| p.get(i).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                if let (Some(i), Some(dm)) = (p.first().and_then(|s| s.parse::<usize>().ok()), self.diag_mut()) {
                    if let Some(c) = dm.connectors.get_mut(i) {
                        c.from_label = p.get(1).unwrap_or(&"").to_string();
                        c.to_label = p.get(2).unwrap_or(&"").to_string();
                        c.from_label_offset = (num(3), num(4));
                        c.to_label_offset = (num(5), num(6));
                    }
                }
            }
            if let Ok(v) = std::env::var("NAPKINUML_SPLIT_AT") {
                let n: Vec<f32> = v.split(',').filter_map(|p| p.parse().ok()).collect();
                let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                if let (Some(d), [x, y]) = (self.diag().cloned(), n.as_slice()) {
                    match seq_menu_target(&d, &opts, 1.0, (*x, *y)) {
                        Some((lid, _, _, Some(ys))) => self.join_activations(lid, ys),
                        Some((lid, yy, true, None)) => self.split_activation(lid, yy),
                        other => self.status = format!("split test: {other:?}"),
                    }
                }
            }
            // for checking drop-to-create: NAPKINUML_DROP_MESSAGE=<message|async|create>,<x>,<y> drops a
            // message from the first lifeline at x (diagram units), pressed at height y
            if let Ok(v) = std::env::var("NAPKINUML_DROP_MESSAGE") {
                let parts: Vec<&str> = v.split(',').collect();
                let kind = match parts.first().copied() { Some("create") => ConnKind::CreateMessage, Some("async") => ConnKind::AsyncMessage, _ => ConnKind::Message };
                let num = |i: usize| parts.get(i).and_then(|p| p.parse::<f32>().ok()).unwrap_or(0.0);
                if let Some(first) = self.diag().and_then(|d| d.shapes.iter().find(|s| s.kind.is_lifeline()).map(|s| s.id)) {
                    self.message_to_new_lifeline(kind, first, num(1), num(2));
                    self.editing = None;
                }
            }
            // for checking self messages: NAPKINUML_SELF_MESSAGE=<message|async>,<lifeline number>,<y>;...
            // adds a self message on that lifeline (1 = first) at height y, as clicking it with the tool does
            if let Ok(v) = std::env::var("NAPKINUML_SELF_MESSAGE") {
                for item in v.split(';') {
                    let parts: Vec<&str> = item.split(',').collect();
                    let kind = if parts.first() == Some(&"async") { ConnKind::AsyncMessage } else { ConnKind::Message };
                    let n = parts.get(1).and_then(|p| p.parse::<usize>().ok()).unwrap_or(1).max(1);
                    let y = parts.get(2).and_then(|p| p.parse::<f32>().ok()).unwrap_or(0.0);
                    if let Some(id) = self.diag().and_then(|d| d.shapes.iter().filter(|s| s.kind.is_lifeline()).nth(n - 1).map(|s| s.id)) {
                        self.create_connector(kind, id, id, y);
                    }
                }
            }
            // for checking printing: NAPKINUML_PRINT_PDF=<file.pdf> prints the first three
            // diagrams to that PDF (paper from NAPKINUML_PRINT_PAPER, A4 by default) and saves
            // the page pictures beside it as PNG
            if let Ok(pdf) = std::env::var("NAPKINUML_PRINT_PDF") {
                let ids: Vec<u64> = self.project.as_ref().map(|p| p.diagrams.iter().take(3).map(|d| d.id).collect()).unwrap_or_default();
                let paper = crate::print::Paper::from_id(&std::env::var("NAPKINUML_PRINT_PAPER").unwrap_or_default());
                self.start_print(ids, PrintTo::Pdf(PathBuf::from(pdf)), paper, crate::print::Orient::Auto);
                // NAPKINUML_JOB_HOLD=1 keeps the job at its first step (to photograph the progress window)
                if std::env::var("NAPKINUML_JOB_HOLD").is_ok() {
                    if let Some(j) = self.printing.as_mut() {
                        j.wait = 250;
                    }
                }
            }
            if let Ok(dir) = std::env::var("NAPKINUML_EXPORT_ALL") {
                let ids: Vec<u64> = self.project.as_ref().map(|p| p.diagrams.iter().map(|d| d.id).collect()).unwrap_or_default();
                // NAPKINUML_EXPORT_FORMAT=png|jpg|webp picks the format
                let format = ImgFormat::from_id(&std::env::var("NAPKINUML_EXPORT_FORMAT").unwrap_or_default());
                self.start_batch_export(ids, PathBuf::from(dir), format);
            }
        }
        if let Some((path, frames)) = self.screenshot.as_mut() {
            *frames += 1;
            // NAPKINUML_SHOT_FRAME=<n> takes the picture at another frame (default 120)
            let at = std::env::var("NAPKINUML_SHOT_FRAME").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(120);
            if *frames == at && self.shot.is_none() {
                self.shot = Some(Shot::Check(path.clone()));
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            if *frames > 900 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }

    }
}

/// Save a picture as PNG, JPEG (quality 92) or lossless WebP, by the file's extension.
/// Move a section line of an alternative fragment to height `ny` (diagram units), keeping it
/// 20 below the line above (or below the fragment's tab) and 20 above the line below (10 above
/// the bottom for the last).
fn set_divider(s: &mut Shape, idx: usize, ny: f32) {
    let mut v = s.alt_dividers();
    if idx < v.len() {
        let lo = if idx == 0 { crate::model::ALT_HEAD } else { v[idx - 1].y + 20.0 };
        let hi = if idx + 1 < v.len() { v[idx + 1].y - 20.0 } else { s.h - 10.0 };
        v[idx].y = (ny - s.y).clamp(lo, hi.max(lo));
        s.dividers = Some(v);
    }
}

/// How close (diagram units, sideways) to its own lifeline a message must be dropped to become
/// a self message; farther away on empty space it makes a new lifeline.
const SELF_REACH: f32 = 50.0;

/// The line between two partitions of a swimlane under a screen point: (swimlane, index of the
/// partition before the line).
fn partition_edge_at(d: &Diagram, cam: &Cam, p: Pos2) -> Option<(u64, usize)> {
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

/// Move what stands inside a swimlane (as it was before a change) beyond a distance along it
/// by `delta`: shapes whose middle is past it and bend points past it.
fn shift_lane_contents(dm: &mut Diagram, lane: &Shape, from: f32, delta: f32) {
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

/// The section line of an alternative fragment under a screen point: (fragment, line index,
/// its height in diagram units).
fn divider_at(d: &Diagram, cam: &Cam, p: Pos2) -> Option<(u64, usize, f32)> {
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

/// The activation bar under a point (diagram units) of a sequence diagram, innermost first.
fn bar_at(d: &Diagram, opts: &crate::model::ProjectOptions, zoom: f32, (x, y): (f32, f32)) -> Option<crate::seq::Bar> {
    if d.kind != DiagramKind::Sequence || !opts.show_activations {
        return None;
    }
    let slack = 4.0 / zoom.max(0.1);
    let lay = crate::seq::layout(d, opts);
    lay.bars
        .iter()
        .filter(|b| y >= b.y0 && y <= b.y1)
        .filter(|b| {
            d.shape(b.lifeline).map(|s| {
                let cx = s.x + s.w / 2.0 + b.depth as f32 * crate::seq::BAR_STEP;
                (x - cx).abs() <= crate::seq::BAR_HALF + slack
            }).unwrap_or(false)
        })
        .max_by_key(|b| b.depth)
        .cloned()
}

/// Move an activation's top or bottom edge to height `ny`. Stretching out adds a stretch
/// (which joins any activation it reaches); pulling the bottom in ends it there with a split;
/// pulling the top back in only takes back stretches (a bar can't start after its message).
fn stretch_activation(s: &mut Shape, depth: usize, top: bool, y0: f32, y1: f32, ny: f32) {
    let dep = depth as f32;
    if top {
        let ny = ny.min(y1 - 10.0);
        if ny < y0 {
            s.act_spans.push([ny, y0 + 1.0, dep]);
        } else {
            for sp in s.act_spans.iter_mut().filter(|sp| sp[2] == dep && sp[0] < ny && sp[1] > y0 - 0.5) {
                sp[0] = ny;
            }
        }
    } else {
        let ny = ny.max(y0 + 10.0);
        if ny > y1 {
            s.act_spans.push([y1 - 1.0, ny, dep]);
        } else {
            for sp in s.act_spans.iter_mut().filter(|sp| sp[2] == dep && sp[1] > ny && sp[0] < y1 + 0.5) {
                sp[1] = ny;
            }
            if ny < y1 - 0.5 {
                s.splits.push(ny);
                s.splits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }
    s.act_spans.retain(|sp| sp[1] - sp[0] > 0.5);
}

/// What a right-click at this point (diagram units) touches in a sequence diagram: the
/// lifeline, the height, whether it is on an activation bar, and a split point near it.
fn seq_menu_target(d: &Diagram, opts: &crate::model::ProjectOptions, zoom: f32, (x, y): (f32, f32)) -> Option<(u64, f32, bool, Option<f32>)> {
    if d.kind != DiagramKind::Sequence {
        return None;
    }
    let lay = crate::seq::layout(d, opts);
    let slack = 4.0 / zoom.max(0.1);
    let bar = lay.bars.iter().filter(|b| y >= b.y0 && y <= b.y1).find(|b| {
        d.shape(b.lifeline).map(|s| {
            let cx = s.x + s.w / 2.0 + b.depth as f32 * crate::seq::BAR_STEP;
            opts.show_activations && (x - cx).abs() <= crate::seq::BAR_HALF + slack
        }).unwrap_or(false)
    });
    let line = d.shapes.iter().filter(|s| s.kind.is_lifeline() && y > s.y && y < s.y + s.h).find(|s| (x - (s.x + s.w / 2.0)).abs() <= crate::seq::BAR_HALF + 3.0 * crate::seq::BAR_STEP + slack);
    let lid = bar.map(|b| b.lifeline).or(line.map(|s| s.id))?;
    let near = d.shape(lid).and_then(|s| s.splits.iter().copied().filter(|v| (v - y).abs() <= 12.0 / zoom.max(0.1)).min_by(|a, b| (a - y).abs().partial_cmp(&(b - y).abs()).unwrap_or(std::cmp::Ordering::Equal)));
    Some((lid, y, bar.is_some(), near))
}

/// In a sequence diagram the lifelines and actors along the top move only sideways, so they
/// stay lined up; a lifeline made by a Create message sits at that message instead.
fn y_locked(d: &Diagram, s: &Shape) -> bool {
    d.kind == DiagramKind::Sequence && s.kind.is_lifeline() && !is_created(d, s.id)
}

/// The lifeline is the target of a Create message.
fn is_created(d: &Diagram, id: u64) -> bool {
    d.connectors.iter().any(|c| c.kind == ConnKind::CreateMessage && c.to == id)
}

/// Height of a lifeline's head box (where a Create message meets it in the middle).
fn lifeline_head(kind: ShapeKind) -> f32 {
    if kind == ShapeKind::Lifeline { 40.0 } else { 70.0 }
}

fn save_picture(image: &image::RgbImage, path: &PathBuf) -> bool {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(dir);
        }
    }
    match ImgFormat::from_path(path).unwrap_or(ImgFormat::Png) {
        ImgFormat::Png => image.save_with_format(path, image::ImageFormat::Png).is_ok(),
        ImgFormat::Jpeg => {
            let Ok(file) = std::fs::File::create(path) else { return false };
            let mut w = std::io::BufWriter::new(file);
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, 92).encode_image(image).is_ok()
        }
        ImgFormat::Webp => {
            let Ok(file) = std::fs::File::create(path) else { return false };
            let mut w = std::io::BufWriter::new(file);
            image::codecs::webp::WebPEncoder::new_lossless(&mut w).encode(image.as_raw(), image.width(), image.height(), image::ExtendedColorType::Rgb8).is_ok()
        }
    }
}

fn save_png(image: &egui::ColorImage, path: &PathBuf) -> bool {
    let [w, h] = image.size;
    let mut bytes = Vec::with_capacity(w * h * 4);
    for p in &image.pixels {
        let [r, g, b, a] = p.to_srgba_unmultiplied();
        bytes.extend_from_slice(&[r, g, b, a]);
    }
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(dir);
        }
    }
    match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
        Some(img) => img.save(path).is_ok(),
        None => false,
    }
}

impl eframe::App for UmlApp {
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
                                if ui.selectable_label(active, &name).clicked() {
                                    open = Some(id);
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
            });
        });
    }
}
