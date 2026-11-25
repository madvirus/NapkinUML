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
        // a timeline axis turned between across and down: its events move with it
        let mut turned = false;
        ui.label(egui::RichText::new(palette::label_for(d.kind, Tool::Shape(s.kind))).strong());
        ui.add_space(4.0);
        egui::Grid::new("shape_props").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            // notes and free text keep their line breaks, so they get a multi-line box
            let multi = matches!(s.kind, ShapeKind::Note | ShapeKind::Text);
            ui.label(tr(lang, if s.kind == ShapeKind::Text { "Text" } else { "Name" }));
            let te = if multi {
                egui::TextEdit::multiline(&mut s.name).desired_rows(4).desired_width(f32::INFINITY)
            } else if s.kind.multiline_name() {
                // a mind map topic's (or a network shape's) name may have line breaks: the box grows with them (Shift+Enter adds one)
                egui::TextEdit::multiline(&mut s.name).desired_rows(1).desired_width(f32::INFINITY).return_key(egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::Enter))
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
            if d.kind == DiagramKind::MindMap && s.kind.is_topic() {
                // the style of the branches to this topic's sub-topics (none: as its parent's)
                let root = crate::mindlayout::is_root(d, s.id);
                let unset = if root { "(Default)" } else { "Same as parent topic" };
                let cur = s.branch_style.and_then(|b| crate::mindlayout::BRANCH_STYLES.iter().find(|(v, _)| *v == b)).map(|(_, l)| *l).unwrap_or(unset);
                ui.label(tr(lang, "Branch Style"));
                egui::ComboBox::from_id_salt("mind_branch_style").selected_text(tr(lang, cur)).show_ui(ui, |ui| {
                    if ui.selectable_label(s.branch_style.is_none(), tr(lang, unset)).clicked() && s.branch_style.is_some() {
                        s.branch_style = None;
                        field_changed = Some(18);
                    }
                    for (v, l) in crate::mindlayout::BRANCH_STYLES {
                        if ui.selectable_label(s.branch_style == Some(v), tr(lang, l)).clicked() && s.branch_style != Some(v) {
                            s.branch_style = Some(v);
                            field_changed = Some(18);
                        }
                    }
                });
                ui.end_row();
            }
            if d.kind == DiagramKind::MindMap && s.kind.is_topic() {
                // the topic's outline: shape, colour and line
                use crate::model::{BORDER_STYLES, TOPIC_SHAPES};
                let own = Shape::new(0, s.kind, 0.0, 0.0, 0.0, 0.0).topic_look();
                let cur = s.topic_look();
                ui.label(tr(lang, "Topic Shape"));
                egui::ComboBox::from_id_salt("mind_topic_shape").selected_text(tr(lang, TOPIC_SHAPES.iter().find(|(v, _)| *v == cur).map(|(_, l)| *l).unwrap_or(""))).show_ui(ui, |ui| {
                    for (v, l) in TOPIC_SHAPES {
                        if ui.selectable_label(cur == v, tr(lang, l)).clicked() && cur != v {
                            // the kind's own shape is kept as no choice, so a pasted topic follows its kind
                            s.topic_shape = if v == own { None } else { Some(v) };
                            field_changed = Some(19);
                        }
                    }
                });
                ui.end_row();
                ui.label(tr(lang, "Border color"));
                ui.horizontal(|ui| {
                    let mut def = s.border_color.is_none();
                    if ui.checkbox(&mut def, tr(lang, "Default")).changed() {
                        s.border_color = if def { None } else { Some(self.look().line().to_array()) };
                        field_changed = Some(20);
                    }
                    if let Some(f) = s.border_color {
                        let mut c = Color32::from_rgba_unmultiplied(f[0], f[1], f[2], f[3]);
                        if ui.color_edit_button_srgba(&mut c).changed() {
                            s.border_color = Some(c.to_array());
                            field_changed = Some(20);
                        }
                    }
                });
                ui.end_row();
                let cur = s.border_style.unwrap_or(crate::model::BORDER_SOLID);
                ui.label(tr(lang, "Border"));
                egui::ComboBox::from_id_salt("mind_border_style").selected_text(tr(lang, BORDER_STYLES.iter().find(|(v, _)| *v == cur).map(|(_, l)| *l).unwrap_or(""))).show_ui(ui, |ui| {
                    for (v, l) in BORDER_STYLES {
                        if ui.selectable_label(cur == v, tr(lang, l)).clicked() && cur != v {
                            s.border_style = if v == crate::model::BORDER_SOLID { None } else { Some(v) };
                            field_changed = Some(21);
                        }
                    }
                });
                ui.end_row();
            }
            if s.kind == ShapeKind::TimeAxis {
                // the period, ticks, direction and look of a timeline's axis
                use crate::model::AXIS_STYLES;
                let mut t = s.timeline();
                let before = t;
                ui.label(tr(lang, "Orientation"));
                egui::ComboBox::from_id_salt("axis_orient").selected_text(tr(lang, if t.vertical { "Vertical" } else { "Horizontal" })).show_ui(ui, |ui| {
                    for (v, l) in [(false, "Horizontal"), (true, "Vertical")] {
                        if ui.selectable_label(t.vertical == v, tr(lang, l)).clicked() {
                            t.vertical = v;
                        }
                    }
                });
                ui.end_row();
                ui.label(tr(lang, "Axis Style"));
                egui::ComboBox::from_id_salt("axis_style").selected_text(tr(lang, AXIS_STYLES.iter().find(|(v, _)| *v == t.style).map(|(_, l)| *l).unwrap_or(""))).show_ui(ui, |ui| {
                    for (v, l) in AXIS_STYLES {
                        if ui.selectable_label(t.style == v, tr(lang, l)).clicked() {
                            t.style = v;
                        }
                    }
                });
                ui.end_row();
                // the tick labels, one per line (an empty line: a tick without a label)
                ui.label(tr(lang, "Ticks"));
                let mut text = s.axis_ticks().join("\n");
                let te = egui::TextEdit::multiline(&mut text).desired_rows(6).desired_width(f32::INFINITY);
                if ui.add(te).on_hover_text(tr(lang, "One tick per line, from the left (top) end to the other; an empty line is a tick without a label.")).changed() {
                    s.attributes = text.split('\n').map(|l| l.to_string()).collect();
                    field_changed = Some(25);
                }
                ui.end_row();
                if t != before {
                    // turning the axis turns its box too
                    if t.vertical != before.vertical {
                        std::mem::swap(&mut s.w, &mut s.h);
                        turned = true;
                    }
                    s.timeline = Some(t);
                    field_changed = Some(23);
                }
            }
            if s.kind == ShapeKind::TimeEvent {
                use crate::model::MARK_STYLES;
                let mut t = s.timeline();
                let before = t;
                ui.label(tr(lang, "Marker"));
                egui::ComboBox::from_id_salt("event_mark").selected_text(tr(lang, MARK_STYLES.iter().find(|(v, _)| *v == t.style).map(|(_, l)| *l).unwrap_or(""))).show_ui(ui, |ui| {
                    for (v, l) in MARK_STYLES {
                        if ui.selectable_label(t.style == v, tr(lang, l)).clicked() {
                            t.style = v;
                        }
                    }
                });
                ui.end_row();
                if t != before {
                    // the event stays where it meets the axis: a flag's line starts at the label's
                    // left edge (or bottom), a dot's at its middle, so the label moves instead
                    let was = crate::model::timeline_stem(d, &s);
                    s.timeline = Some(t);
                    if let (Some(((ax0, ay0), _, _)), Some(((ax1, ay1), _, _))) = (was, crate::model::timeline_stem(d, &s)) {
                        s.x += ax0 - ax1;
                        s.y += ay0 - ay1;
                    }
                    field_changed = Some(23);
                }
                ui.label(tr(lang, "Color"));
                ui.horizontal(|ui| {
                    let mut def = s.border_color.is_none();
                    if ui.checkbox(&mut def, tr(lang, "Default")).changed() {
                        s.border_color = if def { None } else { Some(render::event_color(&s, &self.look()).to_array()) };
                        field_changed = Some(24);
                    }
                    if let Some(f) = s.border_color {
                        let mut c = Color32::from_rgba_unmultiplied(f[0], f[1], f[2], f[3]);
                        if ui.color_edit_button_srgba(&mut c).changed() {
                            s.border_color = Some(c.to_array());
                            field_changed = Some(24);
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
        if s.kind.is_topic() {
            ui.add_space(6.0);
            ui.label(tr(lang, "Markers"));
            if let Some((id, on)) = marker_picker(ui, lang, &s.markers, "prop") {
                let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                let before = render::min_size(ui.ctx(), &s, &opts, &self.look()).x;
                crate::model::set_marker(&mut s.markers, id, on);
                // the topic grows or shrinks by the markers' width (keeping any room the user gave it)
                let after = render::min_size(ui.ctx(), &s, &opts, &self.look()).x;
                s.w = (s.w + after - before).max(after);
                field_changed = Some(22);
            }
        }
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
            let fit = (ui.ctx().clone(), self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default(), self.look());
            if let Some(dm) = self.diag_mut() {
                if turned {
                    crate::model::turn_time_axis(dm, s, |e| render::min_size(&fit.0, e, &fit.1, &fit.2).x);
                } else if let Some(t) = dm.shape_mut(s.id) {
                    *t = s;
                }
            }
        }
    }

    /// Put marker `id` on (`on`) or off the selected topics, as one undo step.
    pub(super) fn set_marker_on_selection(&mut self, ctx: &egui::Context, id: &str, on: bool) {
        let ids: Vec<u64> = self.sel_shapes.clone();
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let look = self.look();
        self.commit_edit();
        self.checkpoint();
        if let Some(d) = self.diag_mut() {
            for s in d.shapes.iter_mut().filter(|s| ids.contains(&s.id) && s.kind.is_topic()) {
                let before = render::min_size(ctx, s, &opts, &look).x;
                crate::model::set_marker(&mut s.markers, id, on);
                let after = render::min_size(ctx, s, &opts, &look).x;
                s.w = (s.w + after - before).max(after);
            }
        }
        if let Some(d) = self.diag_mut() {
            crate::mindlayout::layout(d);
        }
    }

    /// Take every marker off the selected topics, as one undo step.
    pub(super) fn clear_markers_on_selection(&mut self, ctx: &egui::Context) {
        let ids: Vec<u64> = self.sel_shapes.clone();
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let look = self.look();
        self.commit_edit();
        self.checkpoint();
        if let Some(d) = self.diag_mut() {
            for s in d.shapes.iter_mut().filter(|s| ids.contains(&s.id) && s.kind.is_topic()) {
                let before = render::min_size(ctx, s, &opts, &look).x;
                s.markers.clear();
                let after = render::min_size(ctx, s, &opts, &look).x;
                s.w = (s.w + after - before).max(after);
            }
        }
        if let Some(d) = self.diag_mut() {
            crate::mindlayout::layout(d);
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
        // a mind map branch is drawn in the branch style of the topic it leaves
        let mind_branch = d.kind == DiagramKind::MindMap && c.kind == ConnKind::Branch;
        if mind_branch {
            ui.weak(tr(lang, "A branch is drawn in the branch style of the topic it comes from."));
        }
        if !(d.kind == DiagramKind::Sequence && c.kind.is_message()) && !mind_branch {
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
        if !(d.kind == DiagramKind::Sequence && c.kind.is_message()) && !mind_branch {
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
            if kind == DiagramKind::MindMap {
                // adding to the selected topic without the keyboard (the same as Tab and Enter)
                ui.separator();
                let on = self.mind_selected().is_some();
                let (sub, sib) = (self.t("Add Sub-topic").to_string(), self.t("Add Topic").to_string());
                if self.palette_action(ui, true, &sub, "Tab", on, &look) {
                    self.mind_add_clicked(true);
                }
                if self.palette_action(ui, false, &sib, "Enter", on, &look) {
                    self.mind_add_clicked(false);
                }
            }
        });
    }

    /// A palette row that does something at once instead of picking a tool: a mind map's Add
    /// Sub-topic (`child`) or Add Topic. Greyed out (with a hint) while no topic is selected.
    fn palette_action(&mut self, ui: &mut egui::Ui, child: bool, label: &str, key: &str, enabled: bool, look: &Look) -> bool {
        let labels = self.settings.palette_labels;
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), if enabled { Sense::click() } else { Sense::hover() });
        if enabled && resp.hovered() {
            ui.painter().rect_filled(rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let k: f32 = if enabled { 1.0 } else { 0.35 };
        let line = look.line().gamma_multiply(k);
        let fill = look.fill(ShapeKind::Topic).gamma_multiply(k);
        let p = ui.painter();
        let icon = Rect::from_min_size(rect.min + Vec2::new(4.0, 3.0), Vec2::splat(18.0));
        let stroke = egui::Stroke::new(1.0_f32, line);
        // the selected topic, and the new one (filled with the accent colour, a plus inside)
        // beside it (a sub-topic) or under it (a topic next to it)
        let (topic, new) = if child {
            (Rect::from_min_max(icon.min + Vec2::new(0.0, 5.0), icon.min + Vec2::new(6.0, 13.0)), Rect::from_min_max(icon.min + Vec2::new(9.0, 4.0), icon.min + Vec2::new(18.0, 14.0)))
        } else {
            (Rect::from_min_max(icon.min + Vec2::new(1.0, 0.0), icon.min + Vec2::new(14.0, 6.0)), Rect::from_min_max(icon.min + Vec2::new(4.0, 8.0), icon.min + Vec2::new(17.0, 18.0)))
        };
        p.rect(topic, 1.5, fill, stroke, egui::StrokeKind::Middle);
        if child {
            p.line_segment([Pos2::new(topic.right(), topic.center().y), Pos2::new(new.left(), new.center().y)], stroke);
        }
        let accent = ui.visuals().selection.bg_fill.gamma_multiply(k);
        p.rect(new, 2.0, accent, egui::Stroke::NONE, egui::StrokeKind::Middle);
        let ps = egui::Stroke::new(1.6_f32, egui::Color32::WHITE.gamma_multiply(k.max(0.6)));
        let c = new.center();
        p.line_segment([c - Vec2::new(3.0, 0.0), c + Vec2::new(3.0, 0.0)], ps);
        p.line_segment([c - Vec2::new(0.0, 3.0), c + Vec2::new(0.0, 3.0)], ps);
        if labels {
            let color = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
            p.text(Pos2::new(icon.right() + 6.0, rect.center().y), Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0), color);
        }
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        let tip = if enabled { format!("{label} ({key})") } else { format!("{label} ({key})\n{}", self.t("Select a topic first.")) };
        let resp = resp.on_hover_text(tip);
        enabled && resp.clicked()
    }
}

/// The markers a topic can carry, by group, as buttons. Returns the marker clicked and whether it
/// is now on (clicking a marker the topic has takes it off).
pub(super) fn marker_picker(ui: &mut egui::Ui, lang: Language, current: &[String], salt: &str) -> Option<(&'static str, bool)> {
    use crate::model::{MARKERS, MARKER_GROUPS};
    let mut out = None;
    for (g, name) in MARKER_GROUPS.iter().enumerate() {
        ui.weak(tr(lang, name));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
            for d in MARKERS.iter().filter(|d| d.group as usize == g) {
                let on = current.iter().any(|m| m == d.id);
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                let resp = resp.on_hover_text(tr(lang, d.label));
                if on {
                    ui.painter().rect(rect, 4.0, ui.visuals().selection.bg_fill.gamma_multiply(0.35), egui::Stroke::new(1.0_f32, ui.visuals().selection.bg_fill), egui::StrokeKind::Inside);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
                }
                render::draw_marker(ui.painter(), d, rect.center(), 16.0);
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, tr(lang, d.label)));
                if resp.clicked() {
                    out = Some((d.id, !on));
                }
            }
        });
        let _ = salt;
    }
    out
}
