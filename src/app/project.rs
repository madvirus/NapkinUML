//! Project and diagram management: new/open/save/close, windows, undo and redo.

use super::*;

/// Set on windows this program opens itself (Window > New Window...), so only the first
/// window checks for updates.
pub(super) const CHILD_WINDOW_ENV: &str = "NAPKINUML_CHILD_WINDOW";

impl UmlApp {
    pub(super) fn open_diagram(&mut self, id: u64) {
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

    pub(super) fn close_diagram(&mut self, id: u64) {
        let idx = self.tabs.iter().position(|t| *t == id);
        self.tabs.retain(|t| *t != id);
        if self.active == Some(id) {
            self.active = idx.and_then(|i| self.tabs.get(i.min(self.tabs.len().saturating_sub(1))).copied()).or(self.tabs.last().copied());
            self.clear_selection();
            self.editing = None;
        }
    }

    pub(super) fn new_project(&mut self) {
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

    pub(super) fn add_diagram(&mut self, kind: DiagramKind, name: Option<String>) {
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

    pub(super) fn delete_diagram(&mut self, id: u64) {
        self.close_diagram(id);
        if let Some(p) = self.project.as_mut() {
            p.diagrams.retain(|d| d.id != id);
        }
        self.undo.retain(|(d, _)| *d != id);
        self.redo.retain(|(d, _)| *d != id);
        self.dirty = true;
    }

    pub(super) fn duplicate_diagram(&mut self, id: u64) {
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

    pub(super) fn save_to(&mut self, path: PathBuf) -> bool {
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

    pub(super) fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(p) => self.save_to(p),
            None => self.save_as(),
        }
    }

    pub(super) fn save_as(&mut self) -> bool {
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

    pub(super) fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("UML Project", &[FILE_EXT, "json"]).pick_file() {
            self.open_path(p);
        }
    }

    pub(super) fn open_path(&mut self, path: PathBuf) {
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
                self.fonts.request_refresh();
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

    pub(super) fn close_project(&mut self) {
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
    pub(super) fn request(&mut self, action: Pending) {
        if self.dirty && self.project.is_some() && self.screenshot.is_none() {
            self.confirm = Some(action);
        } else {
            self.perform(action);
        }
    }

    pub(super) fn perform(&mut self, action: Pending) {
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
    pub(super) fn open_example(&mut self) {
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
        self.fonts.request_refresh();
        self.dirty = false;
    }

    /// Pick a project file and open it in a new window (this window keeps its project).
    pub(super) fn open_in_new_window(&mut self) {
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

    pub(super) fn new_window(&mut self) {
        match std::env::current_exe() {
            Ok(exe) => match std::process::Command::new(exe).arg("--new").env(CHILD_WINDOW_ENV, "1").spawn() {
                Ok(_) => self.status = "Opened a new window".into(),
                Err(e) => self.status = format!("Could not open a new window: {e}"),
            },
            Err(e) => self.status = format!("Could not open a new window: {e}"),
        }
    }

    pub(super) fn checkpoint(&mut self) {
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

    pub(super) fn edit_checkpoint(&mut self, key: (u64, u8)) {
        if self.prop_session != Some(key) {
            self.checkpoint();
            self.prop_session = Some(key);
        }
        self.dirty = true;
    }

    pub(super) fn undo(&mut self) {
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

    pub(super) fn redo(&mut self) {
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
}
