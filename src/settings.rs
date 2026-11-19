//! Application options, saved per user (Window > Application Options).

use crate::i18n::Language;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub language: Language,
    pub dark: bool,
    pub grid_size: f32,
    pub show_grid: bool,
    pub snap: bool,
    pub text_size: f32,
    pub confirm_delete: bool,
    pub palette_labels: bool,
    pub recent_max: usize,
    pub recent: Vec<String>,
    pub first_run_done: bool,
    /// Font family for diagram text ("" = the built-in font).
    pub font: String,
    /// AI Assistant: which agent to run ("" = the first one found, "custom" = ai_command).
    pub ai_agent: String,
    /// AI Assistant: the command line of a custom agent (the request goes to its input).
    pub ai_command: String,
    /// AI Assistant: the Ollama model ("" = the first one installed).
    pub ai_model: String,
    /// Print: the paper last chosen ("A4", "A3"...) and orientation (0 auto, 1 portrait, 2 landscape).
    pub print_paper: String,
    pub print_orient: u8,
    /// Export: the picture format last chosen ("png", "jpg" or "webp").
    pub export_format: String,
    /// Ask GitHub for a newer version after start.
    pub check_updates: bool,
    /// A version the person chose to skip ("" = none): it is not offered again.
    pub skip_version: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: Language::System,
            dark: false,
            // v.ddg.r.<init>: grid 10 x 10 (Ii, Ij), ShowGrid false (Ig), AlignToGrid false (Ho)
            grid_size: 10.0,
            show_grid: false,
            snap: false,
            // DefaultFont is Dialog plain 11 (v.ddg.r.<clinit>, field ab -> HF)
            text_size: 11.0,
            confirm_delete: true,
            palette_labels: true,
            recent_max: 8,
            recent: Vec::new(),
            first_run_done: false,
            font: String::new(),
            ai_agent: String::new(),
            ai_command: String::new(),
            ai_model: String::new(),
            print_paper: "A4".into(),
            print_orient: 0,
            export_format: "png".into(),
            check_updates: true,
            skip_version: String::new(),
        }
    }
}

/// Set during a --screenshot check: settings are never written then.
pub static READ_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn settings_base() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(PathBuf::from))
}

fn settings_path() -> Option<PathBuf> {
    Some(settings_base()?.join("NapkinUML").join("settings.json"))
}

/// Where the program kept its settings before it was renamed (Simple UML).
fn old_settings_path() -> Option<PathBuf> {
    Some(settings_base()?.join("SimpleUML").join("settings.json"))
}

impl Settings {
    pub fn load() -> Settings {
        settings_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .or_else(|| old_settings_path().and_then(|p| std::fs::read_to_string(p).ok()))
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if READ_ONLY.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        if let Some(p) = settings_path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(t) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, t);
            }
        }
    }

    pub fn add_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(self.recent_max.max(1));
    }
}
