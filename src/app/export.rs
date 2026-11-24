//! Printing and image export (single picture, clipboard and multiple diagrams).

use super::*;

/// The "export several diagrams" dialog: which diagrams, and the folder to write them to.
struct ExportDlg {
    chosen: Vec<(u64, bool)>,
    folder: Option<PathBuf>,
    pk: Picker,
    format: ImgFormat,
}

/// The picture formats Export can write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ImgFormat {
    Png,
    Jpeg,
    Webp,
}

impl ImgFormat {
    const ALL: [ImgFormat; 3] = [ImgFormat::Png, ImgFormat::Jpeg, ImgFormat::Webp];
    pub(super) fn ext(self) -> &'static str {
        match self {
            ImgFormat::Png => "png",
            ImgFormat::Jpeg => "jpg",
            ImgFormat::Webp => "webp",
        }
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            ImgFormat::Png => "PNG",
            ImgFormat::Jpeg => "JPEG",
            ImgFormat::Webp => "WebP",
        }
    }
    /// What the format is good for, shown beside the choice.
    pub(super) fn note(self) -> &'static str {
        match self {
            ImgFormat::Png => "Sharp, no loss (best for diagrams)",
            ImgFormat::Jpeg => "Small files, slight blur on lines (quality 92)",
            ImgFormat::Webp => "Sharp, no loss, smaller than PNG",
        }
    }
    pub(super) fn exts(self) -> &'static [&'static str] {
        match self {
            ImgFormat::Png => &["png"],
            ImgFormat::Jpeg => &["jpg", "jpeg"],
            ImgFormat::Webp => &["webp"],
        }
    }
    pub(super) fn from_id(s: &str) -> ImgFormat {
        match s.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => ImgFormat::Jpeg,
            "webp" => ImgFormat::Webp,
            _ => ImgFormat::Png,
        }
    }
    pub(super) fn from_path(p: &std::path::Path) -> Option<ImgFormat> {
        let e = p.extension()?.to_str()?.to_ascii_lowercase();
        ImgFormat::ALL.into_iter().find(|f| f.exts().contains(&e.as_str()))
    }
}

/// Search text and type filter of the diagram picker (Print and Export dialogs).
#[derive(Default)]
pub(super) struct Picker {
    pub(super) search: String,
    pub(super) kind: Option<DiagramKind>,
}

/// Diagrams under a folder (and its subfolders) that pass the picker's filters; None = everything.
pub(super) fn picked_under(p: &Project, folder: Option<u64>, visible: &std::collections::HashSet<u64>) -> Vec<u64> {
    let mut out: Vec<u64> = p.diagrams.iter().filter(|d| visible.contains(&d.id) && d.folder.filter(|f| p.folder(*f).is_some()) == folder).map(|d| d.id).collect();
    for sub in p.folders.iter().filter(|x| x.parent == folder) {
        out.extend(picked_under(p, Some(sub.id), visible));
    }
    out
}

/// One folder of the picker: a check box for everything inside it (a dash when only some
/// are ticked), its subfolders, then its diagrams. The root (None) has no header.
#[allow(clippy::too_many_arguments)]
pub(super) fn picker_folder(
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
pub(super) fn diagram_picker(ui: &mut egui::Ui, salt: &str, lang: Language, p: &Project, chosen: &mut [(u64, bool)], pk: &mut Picker, open_tabs: &[u64], current: Option<u64>, height: f32) {
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

/// Project > Export > Multiple Diagrams and Project > Print: each dialog, then the work it started.
#[derive(Default)]
pub(super) struct Jobs {
    export_dlg: Option<ExportDlg>,
    batch: Option<BatchExport>,
    print_dlg: Option<PrintDlg>,
    printing: Option<PrintJob>,
    /// The progress window's Cancel was pressed.
    cancel: bool,
}

impl Jobs {
    /// A dialog or progress window of these is open.
    pub(super) fn any_open(&self) -> bool {
        self.export_dlg.is_some() || self.batch.is_some() || self.print_dlg.is_some() || self.printing.is_some()
    }

    /// NAPKINUML_JOB_HOLD: keep printing at its first step (to photograph the progress window).
    pub(super) fn hold(&mut self) {
        if let Some(j) = self.printing.as_mut() {
            j.wait = 250;
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
pub(super) enum PrintTo {
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
pub(super) fn file_name_for(name: &str) -> String {
    let mut s: String = name.chars().map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c }).collect();
    s = s.trim().trim_end_matches('.').to_string();
    if s.is_empty() {
        s = "diagram".into();
    }
    s
}

pub(super) fn save_picture(image: &image::RgbImage, path: &PathBuf) -> bool {
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

pub(super) fn save_png(image: &egui::ColorImage, path: &PathBuf) -> bool {
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

impl UmlApp {
    /// For checking the Print dialog (NAPKINUML_PRINT_DLG=1, with NAPKINUML_PICK_SEARCH as its
    /// search text) and the multi-image export dialog (NAPKINUML_EXPORT_DLG=1) in pictures.
    pub(super) fn export_test_hooks(&mut self) {
        if std::env::var("NAPKINUML_PRINT_DLG").is_ok() {
            self.open_print_dialog();
            if let (Ok(s), Some(d)) = (std::env::var("NAPKINUML_PICK_SEARCH"), self.jobs.print_dlg.as_mut()) {
                d.pk.search = s;
            }
        }
        if std::env::var("NAPKINUML_EXPORT_DLG").is_ok() {
            self.open_export_dialog();
        }
    }

    pub(super) fn open_print_dialog(&mut self) {
        let Some(p) = self.project.as_ref() else { return };
        if self.active.is_none() {
            return;
        }
        // the open diagram is ticked; others can be added (each prints on its own page)
        let chosen = p.diagrams.iter().map(|d| (d.id, Some(d.id) == self.active)).collect();
        self.jobs.print_dlg = Some(PrintDlg { paper: crate::print::Paper::from_id(&self.settings.print_paper), orient: crate::print::Orient::from_code(self.settings.print_orient), chosen, pk: Picker::default() });
    }

    pub(super) fn print_dialog(&mut self, ctx: &egui::Context) {
        use crate::print::{Orient, Paper};
        let Some(mut dlg) = self.jobs.print_dlg.take() else { return };
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
            self.jobs.print_dlg = Some(dlg);
        }
    }

    pub(super) fn start_print(&mut self, ids: Vec<u64>, to: PrintTo, paper: crate::print::Paper, orient: crate::print::Orient) {
        if ids.is_empty() {
            return;
        }
        self.commit_edit();
        self.menu = None;
        let total = ids.len();
        self.jobs.printing = Some(PrintJob { to, paper, orient, queue: ids.into_iter().collect(), total, pages: vec![], wait: 3, finishing: false });
        self.jobs.cancel = false;
        self.status = format!("{} 1/{total}", tr(self.settings.language, "Preparing pages"));
    }

    /// The picture of one diagram drawn off screen, at `scale` pixels per unit and at most
    /// `max_px` on its longer side.
    fn picture_of(&self, id: u64, scale: f32, max_px: f32) -> Option<image::RgbImage> {
        let p = self.project.as_ref()?;
        let d = p.diagram(id)?;
        Some(crate::offscreen::diagram_picture(d, &p.options, &self.look(), self.fonts.defs(), scale, max_px))
    }

    /// Printing and exporting several diagrams: one diagram each frame, so the window keeps
    /// answering and shows how far it got. Esc stops.
    pub(super) fn work_jobs(&mut self, ctx: &egui::Context) {
        let lang = self.settings.language;
        let esc = ctx.input(|i| i.key_pressed(Key::Escape)) || std::mem::take(&mut self.jobs.cancel);
        if self.jobs.printing.is_some() {
            if esc {
                self.jobs.printing = None;
                self.status = tr(lang, "Printing cancelled.").to_string();
                return;
            }
            ctx.request_repaint();
            // let the progress window show first (and say what comes next)
            if let Some(j) = self.jobs.printing.as_mut() {
                if j.wait > 0 {
                    j.wait -= 1;
                    return;
                }
                if j.finishing {
                    self.finish_print();
                    return;
                }
            }
            let next = self.jobs.printing.as_mut().and_then(|j| j.queue.pop_front());
            let paper = self.jobs.printing.as_ref().map(|j| j.paper).unwrap_or(crate::print::Paper::A4);
            if let Some(id) = next {
                // sharp on paper: about 250 dots per inch over the printable length
                if let Some(pic) = self.picture_of(id, 8.0, crate::print::target_long_px(paper)) {
                    if let Some(j) = self.jobs.printing.as_mut() {
                        j.pages.push(pic);
                    }
                }
            }
            if let Some(j) = self.jobs.printing.as_mut() {
                if j.queue.is_empty() {
                    j.finishing = true;
                    j.wait = 2;
                } else {
                    self.status = format!("{} {}/{}", tr(lang, "Preparing pages"), j.pages.len() + 1, j.total);
                }
            }
            return;
        }
        if self.jobs.batch.is_some() {
            ctx.request_repaint();
            if esc {
                if let Some(b) = self.jobs.batch.as_mut() {
                    b.queue.clear();
                }
            }
            if let Some(b) = self.jobs.batch.as_mut() {
                if b.wait > 0 && !b.queue.is_empty() {
                    b.wait -= 1;
                    return;
                }
            }
            let next = self.jobs.batch.as_mut().and_then(|b| b.queue.pop_front());
            if let Some((id, path)) = next {
                let ok = self.picture_of(id, 2.0, 8000.0).map(|pic| save_picture(&pic, &path)).unwrap_or(false);
                if let Some(b) = self.jobs.batch.as_mut() {
                    if ok {
                        b.done += 1;
                    } else {
                        b.failed += 1;
                    }
                }
            }
            if let Some(b) = self.jobs.batch.as_ref() {
                if b.queue.is_empty() {
                    let b = self.jobs.batch.take().unwrap();
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
    pub(super) fn job_progress(&mut self, ctx: &egui::Context) {
        let lang = self.settings.language;
        let t = |s: &str| tr(lang, s).to_string();
        let (title, line, done, total) = if let Some(j) = &self.jobs.printing {
            let to_pdf = matches!(j.to, PrintTo::Pdf(_));
            let title = t(if to_pdf { "Saving as PDF..." } else { "Preparing to print..." });
            let line = if j.finishing {
                t(if to_pdf { "Writing the PDF file..." } else { "Opening the Print dialog..." })
            } else {
                format!("{} {} / {}", t("Preparing page"), (j.pages.len() + 1).min(j.total), j.total)
            };
            (title, line, j.pages.len(), j.total)
        } else if let Some(b) = &self.jobs.batch {
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
            self.jobs.cancel = true;
        }
    }

    /// Every page is ready: print it or write the PDF.
    fn finish_print(&mut self) {
        let Some(job) = self.jobs.printing.take() else { return };
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

    pub(super) fn open_export_dialog(&mut self) {
        let Some(p) = self.project.as_ref() else { return };
        // every diagram is ticked at first; the folder starts beside the project file
        let chosen = p.diagrams.iter().map(|d| (d.id, true)).collect();
        let folder = self.path.as_ref().and_then(|f| f.parent().map(|d| d.to_path_buf()));
        let format = ImgFormat::from_id(&self.settings.export_format);
        self.jobs.export_dlg = Some(ExportDlg { chosen, folder, pk: Picker::default(), format });
    }

    pub(super) fn export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.jobs.export_dlg.take() else { return };
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
            self.jobs.export_dlg = Some(dlg);
        }
    }

    /// Queue the diagrams; work_jobs draws and saves one per frame.
    pub(super) fn start_batch_export(&mut self, ids: Vec<u64>, folder: PathBuf, format: ImgFormat) {
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
        self.jobs.batch = Some(BatchExport { queue, total, done: 0, failed: 0, folder, wait: 3 });
        self.jobs.cancel = false;
    }

    /// The selection (with what sits inside selected containers, attached ports, joined
    /// interfaces and the lines among them), or the whole diagram when nothing is selected,
    /// put on the clipboard as a picture (twice the 100% size) for pasting into PowerPoint,
    /// Word and the like.
    pub(super) fn copy_as_image(&mut self) {
        let lang = self.settings.language;
        let Some((sub, picked)) = self.copy_subject() else { return };
        let Some(p) = self.project.as_ref() else { return };
        let pic = crate::offscreen::diagram_picture(&sub, &p.options, &self.look(), self.fonts.defs(), 2.0, 6000.0);
        let (w, h) = pic.dimensions();
        let rgba: Vec<u8> = pic.pixels().flat_map(|px| [px[0], px[1], px[2], 255]).collect();
        let result = arboard::Clipboard::new().and_then(|mut cb| cb.set_image(arboard::ImageData { width: w as usize, height: h as usize, bytes: std::borrow::Cow::Owned(rgba) }));
        self.status = match result {
            Ok(()) => format!("{} ({w} x {h})", tr(lang, if picked { "The selection was copied as an image" } else { "The diagram was copied as an image" })),
            Err(e) => format!("{}: {e}", tr(lang, "Could not copy the image")),
        };
    }

    /// The selection (or the whole diagram) put on the clipboard as an editable picture (EMF):
    /// pasted into PowerPoint (or Word, Excel), it can be ungrouped into the program's own shapes,
    /// lines and text boxes. `save_to` also writes the file (for checking).
    pub(super) fn copy_for_office(&mut self, save_to: Option<&std::path::Path>) {
        let lang = self.settings.language;
        let Some((sub, picked)) = self.copy_subject() else { return };
        let Some(p) = self.project.as_ref() else { return };
        let bytes = crate::offscreen::diagram_emf(&sub, &p.options, &self.look(), self.fonts.defs());
        if let Some(path) = save_to {
            let _ = std::fs::write(path, &bytes);
        }
        self.status = match crate::emf::to_clipboard(&bytes) {
            Ok(()) => format!(
                "{} {}",
                tr(lang, if picked { "The selection was copied as editable shapes." } else { "The diagram was copied as editable shapes." }),
                tr(lang, "In PowerPoint, paste it, then Ungroup it (Ctrl+Shift+G) to edit each shape.")
            ),
            Err(e) => format!("{}: {e}", tr(lang, "Could not copy the shapes")),
        };
    }

    /// What Copy as Image (and as EMF) copies: the selection (with what sits inside
    /// selected containers, attached ports, joined interfaces and the lines among them), or the
    /// whole diagram when nothing is selected; and whether it is a selection.
    fn copy_subject(&mut self) -> Option<(Diagram, bool)> {
        let lang = self.settings.language;
        let d = self.diag()?;
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
            return None;
        }
        Some((sub, picked))
    }

    pub(super) fn export_image(&mut self) {
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
}
