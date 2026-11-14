//! Fonts for diagram text: the built-in font plus common Windows fonts, loaded on demand.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// Fonts offered in the font lists (display name, file in the Windows font folder).
const CANDIDATES: &[(&str, &str)] = &[
    ("Malgun Gothic", "malgun.ttf"),
    ("Gulim", "gulim.ttc"),
    ("Batang", "batang.ttc"),
    ("Segoe UI", "segoeui.ttf"),
    ("Arial", "arial.ttf"),
    ("Calibri", "calibri.ttf"),
    ("Verdana", "verdana.ttf"),
    ("Tahoma", "tahoma.ttf"),
    ("Times New Roman", "times.ttf"),
    ("Georgia", "georgia.ttf"),
    ("Cambria", "cambria.ttc"),
    ("Consolas", "consola.ttf"),
    ("Courier New", "cour.ttf"),
    ("Comic Sans MS", "comic.ttf"),
];

fn font_dir() -> PathBuf {
    let win = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    win.join("Fonts")
}

fn file_for(name: &str) -> Option<PathBuf> {
    CANDIDATES.iter().find(|(n, _)| *n == name).map(|(_, f)| font_dir().join(f))
}

/// Font names whose files exist on this computer.
pub fn available() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| CANDIDATES.iter().filter(|(_, f)| font_dir().join(f).exists()).map(|(n, _)| *n).collect())
}

/// Font definitions: egui's built-in fonts, a Hangul fallback, and each wanted font as
/// its own family (falling back to the built-in fonts for missing characters).
/// Returns the definitions and the names that really loaded.
pub fn build(wanted: &[String]) -> (egui::FontDefinitions, Vec<String>) {
    let mut fonts = egui::FontDefinitions::default();
    // Korean text needs a Hangul font; Windows ships Malgun Gothic.
    let hangul = [font_dir().join("malgun.ttf"), PathBuf::from("/System/Library/Fonts/AppleSDGothicNeo.ttc"), PathBuf::from("/usr/share/fonts/truetype/nanum/NanumGothic.ttf")];
    for c in hangul {
        if let Ok(bytes) = std::fs::read(&c) {
            fonts.font_data.insert("hangul".to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(fam).or_default().push("hangul".to_owned());
            }
            break;
        }
    }
    let base = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    let mut loaded = vec![];
    for name in wanted {
        if name.is_empty() || loaded.contains(name) {
            continue;
        }
        let Some(path) = file_for(name) else { continue };
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let key = format!("font:{name}");
        fonts.font_data.insert(key.clone(), Arc::new(egui::FontData::from_owned(bytes)));
        let mut list = vec![key];
        list.extend(base.iter().cloned());
        fonts.families.insert(egui::FontFamily::Name(name.as_str().into()), list);
        loaded.push(name.clone());
    }
    (fonts, loaded)
}
