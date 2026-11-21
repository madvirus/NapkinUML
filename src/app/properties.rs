//! The property pane and the diagram palette.

use super::*;

/// Twelve pastel fill colours offered as presets.
pub(super) const PASTELS: [(&str, [u8; 3]); 18] = [
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
pub(super) fn pastel_swatches(ui: &mut egui::Ui, lang: Language, current: Option<[u8; 4]>) -> Option<[u8; 4]> {
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
pub(super) fn navigable(kind: ConnKind) -> bool {
    matches!(kind, ConnKind::Association | ConnKind::Aggregation | ConnKind::Composition)
}

impl UmlApp {
    pub(super) fn property_pane(&mut self, ui: &mut egui::Ui) {
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
                    self.fonts.request_refresh();
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
    pub(super) fn diagram_numbering(&mut self, ui: &mut egui::Ui, d: &Diagram) {
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

    pub(super) fn shape_properties(&mut self, ui: &mut egui::Ui, d: &Diagram, mut s: Shape) {
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
            if matches!(s.kind, ShapeKind::Class | ShapeKind::Interface) {
                // per class or interface: hide the attribute and/or operation compartment
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
            if d.kind == DiagramKind::MindMap && crate::mindlayout::is_root(d, s.id) {
                // a root topic picks how the topics below it are laid out (XMind's structures)
                let cur = crate::mindlayout::STRUCTURES.iter().find(|(v, _)| *v == s.orient.min(2)).map(|(_, l)| *l).unwrap_or("Mind Map");
                ui.label(tr(lang, "Structure"));
                egui::ComboBox::from_id_salt("mind_structure").selected_text(tr(lang, cur)).show_ui(ui, |ui| {
                    for (v, l) in crate::mindlayout::STRUCTURES {
                        if ui.selectable_label(s.orient.min(2) == v, tr(lang, l)).clicked() && s.orient != v {
                            s.orient = v;
                            field_changed = Some(11);
                        }
                    }
                });
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
                self.fonts.request_refresh();
            }
            if f == 14 {
                // hiding or showing a compartment fits the class (or interface) height to what is left
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

    pub(super) fn conn_properties(&mut self, ui: &mut egui::Ui, d: &Diagram, mut c: Connector) {
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

    pub(super) fn palette_row(&mut self, ui: &mut egui::Ui, tool: Tool, label: &str, tip: &str, look: &Look, opts: &ProjectOptions) {
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

    pub(super) fn palette_pane(&mut self, ui: &mut egui::Ui, kind: DiagramKind) {
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
}
