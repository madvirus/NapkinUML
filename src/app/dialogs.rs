//! Dialogs: About, New Diagram, options, switcher, rename and confirmations.

use super::*;

pub(super) struct AppOptionsDlg {
    pub(super) s: Settings,
    pub(super) page: usize,
}

pub(super) struct ProjectOptionsDlg {
    pub(super) o: ProjectOptions,
    pub(super) name: String,
    pub(super) author: String,
    pub(super) description: String,
    pub(super) page: usize,
}

/// Make the dialog being drawn modal: Tab and Shift+Tab move the keyboard focus through its
/// boxes and buttons only (Enter or Space presses a focused button), and the window behind
/// it can't be clicked. Takes effect from the next frame.
pub(super) fn modal_dialog(ui: &egui::Ui) {
    let layer = ui.layer_id();
    ui.ctx().memory_mut(|m| m.set_modal_layer(layer));
}

/// Esc pressed while no drop-down list or colour picker is open (Esc closes those first).
pub(super) fn esc_cancels(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.key_pressed(Key::Escape)) && !ctx.memory(|m| m.any_popup_open())
}

impl UmlApp {
    pub(super) fn open_project_options(&mut self, page: usize) {
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

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
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
        self.about_dialog(ctx);
    }

    /// Help > About NapkinUML: the icon, the version, the licence and the project's site.
    pub(super) fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let lang = self.settings.language;
        let t = |s: &'static str| tr(lang, s);
        if self.about_icon.is_none() {
            let bytes: &[u8] = include_bytes!("../../icons/napkin-uml-256.png");
            if let Ok(img) = image::load_from_memory_with_format(bytes, image::ImageFormat::Png) {
                let img = img.to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                self.about_icon = Some(ctx.load_texture("about_icon", color, egui::TextureOptions::LINEAR));
            }
        }
        let mut open = true;
        let mut close = esc_cancels(ctx);
        egui::Window::new(t("About NapkinUML"))
            .id(egui::Id::new("about_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    if let Some(tex) = &self.about_icon {
                        ui.add(egui::Image::new((tex.id(), Vec2::splat(96.0))));
                    }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(format!("NapkinUML {}", env!("CARGO_PKG_VERSION"))).size(20.0).strong());
                    ui.add_space(10.0);
                    ui.label(format!("{} {}", t("License:"), env!("CARGO_PKG_LICENSE")));
                    ui.label(egui::RichText::new(t("Use it under either the MIT or the Apache 2.0 license.")).weak());
                    ui.add_space(10.0);
                    let site = env!("CARGO_PKG_REPOSITORY");
                    ui.label(t("GitHub:"));
                    ui.hyperlink_to(site, site);
                    ui.add_space(12.0);
                    if ui.button(format!("   {}   ", t("OK"))).clicked() {
                        close = true;
                    }
                    ui.add_space(4.0);
                });
            });
        if !open || close {
            self.about_open = false;
        }
    }

    pub(super) fn new_diagram_dialog(&mut self, ctx: &egui::Context) {
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

    pub(super) fn app_options_dialog(&mut self, ctx: &egui::Context) {
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
            self.fonts.request_refresh();
        }
        if esc && !ok {
            cancel = true;
        }
        if ok || cancel || !open {
            self.app_opts = None;
        }
    }

    pub(super) fn project_options_dialog(&mut self, ctx: &egui::Context) {
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
                self.fonts.request_refresh();
            }
        }
        if esc && !ok {
            cancel = true;
        }
        if ok || cancel || !open {
            self.proj_opts = None;
        }
    }

    pub(super) fn switcher_dialog(&mut self, ctx: &egui::Context) {
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

    pub(super) fn rename_dialog(&mut self, ctx: &egui::Context) {
        let Some((id, mut name)) = self.rename.clone() else { return };
        let lang = self.settings.language;
        let mut done = false;
        let mut cancel = false;
        // the first frame of this dialog puts the cursor in the box with the name selected
        let first = self.rename_focused != Some(id);
        egui::Window::new(tr(lang, if id == PROJECT_RENAME_ID { "Rename Project" } else { "Rename" }))
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
                    if id == PROJECT_RENAME_ID {
                        p.name = name.trim().to_string();
                        self.dirty = true;
                    } else if let Some(f) = p.folders.iter_mut().find(|f| f.id == id) {
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

    pub(super) fn confirm_dialogs(&mut self, ctx: &egui::Context) {
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
}
