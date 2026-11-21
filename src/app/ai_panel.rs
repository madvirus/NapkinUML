//! The AI Assistant pane and its conversation with the agent.

use super::*;

/// The AI Assistant panel and its conversation (true = the person's message).
pub(super) struct AiPanel {
    pub(super) show: bool,
    agents: Vec<crate::ai::Found>,
    chat: Vec<(bool, String)>,
    input: String,
    job: Option<crate::ai::Job>,
}

impl AiPanel {
    pub(super) fn new() -> Self {
        AiPanel { show: false, agents: crate::ai::detect(), chat: vec![], input: String::new(), job: None }
    }

    /// The agent chosen in the panel, if it can run.
    fn agent(&self, settings: &Settings) -> Option<crate::ai::Agent> {
        if settings.ai_agent == "custom" {
            return Some(crate::ai::Agent::Custom(settings.ai_command.clone())).filter(|_| !settings.ai_command.trim().is_empty());
        }
        self.agents
            .iter()
            .find(|f| f.id == settings.ai_agent)
            .or_else(|| self.agents.first())
            .cloned()
            .map(crate::ai::Agent::Known)
    }
}

impl UmlApp {
    /// For checking the AI Assistant in pictures: NAPKINUML_AI_PANEL=1, and
    /// NAPKINUML_AI_REPLY=<file> applies a saved agent answer as if it had just arrived.
    pub(super) fn ai_test_hooks(&mut self) {
        if std::env::var("NAPKINUML_AI_PANEL").is_ok() {
            self.ai.show = true;
        }
        if let Ok(f) = std::env::var("NAPKINUML_AI_REPLY") {
            if let Ok(text) = std::fs::read_to_string(f) {
                self.ai.show = true;
                self.ai.chat.push((true, "(test answer)".into()));
                self.ai_answer(Ok(text));
            }
        }
    }

    fn ai_send(&mut self) {
        let request = self.ai.input.trim().to_string();
        if request.is_empty() || self.ai.job.is_some() {
            return;
        }
        let Some(agent) = self.ai.agent(&self.settings) else {
            self.ai.chat.push((false, tr(self.settings.language, "No AI agent is set. Install Claude Code, Codex CLI, Gemini CLI or Ollama, or set a custom command.").to_string()));
            return;
        };
        let prompt = crate::ai::build_prompt(self.project.as_ref(), self.diag(), &self.ai.chat, &request);
        self.ai.chat.push((true, request));
        self.ai.input.clear();
        self.ai.job = Some(crate::ai::start(agent, self.settings.ai_model.clone(), prompt));
    }

    pub(super) fn ai_poll(&mut self, ctx: &egui::Context) {
        let Some(job) = self.ai.job.as_ref() else { return };
        match job.rx.try_recv() {
            Ok(result) => {
                self.ai.job = None;
                self.ai_answer(result);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(250)),
            Err(_) => {
                self.ai.job = None;
                self.ai.chat.push((false, "The AI agent stopped unexpectedly.".into()));
            }
        }
    }

    /// Show the agent's answer and put its diagram on the canvas (undo takes it back).
    fn ai_answer(&mut self, result: Result<String, String>) {
        let lang = self.settings.language;
        let text = match result {
            Ok(t) => t,
            Err(e) => {
                self.ai.chat.push((false, e));
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
                    self.fonts.request_refresh();
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
        self.ai.chat.push((false, message));
    }

    pub(super) fn ai_pane(&mut self, ui: &mut egui::Ui) {
        let lang = self.settings.language;
        let t = |s: &str| tr(lang, s).to_string();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading(t("AI Assistant"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("×").on_hover_text(t("Close")).clicked() {
                    self.ai.show = false;
                }
                if !self.ai.chat.is_empty() && self.ai.job.is_none() && ui.small_button(t("Clear")).clicked() {
                    self.ai.chat.clear();
                }
            });
        });
        ui.separator();
        // which agent
        let mut choice = self.settings.ai_agent.clone();
        if choice.is_empty() || (choice != "custom" && !self.ai.agents.iter().any(|f| f.id == choice)) {
            choice = self.ai.agents.first().map(|f| f.id.to_string()).unwrap_or_else(|| "custom".into());
        }
        let label = if choice == "custom" { t("Custom command") } else { self.ai.agents.iter().find(|f| f.id == choice).map(|f| f.label.to_string()).unwrap_or_default() };
        ui.horizontal(|ui| {
            ui.label(t("Agent"));
            egui::ComboBox::from_id_salt("ai_agent").selected_text(label).width(170.0).show_ui(ui, |ui| {
                for f in &self.ai.agents {
                    ui.selectable_value(&mut choice, f.id.to_string(), f.label).on_hover_text(f.path.display().to_string());
                }
                ui.selectable_value(&mut choice, "custom".to_string(), t("Custom command"));
            });
            if ui.small_button("⟳").on_hover_text(t("Look for installed agents again")).clicked() {
                self.ai.agents = crate::ai::detect();
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
        if self.ai.agents.is_empty() && choice != "custom" {
            ui.colored_label(Color32::from_rgb(200, 90, 40), t("No AI agent was found on this computer. Install Claude Code, Codex CLI, Gemini CLI or Ollama, or set a custom command."));
        }
        ui.separator();
        // the request box sits at the bottom, the conversation fills the rest
        egui::TopBottomPanel::bottom("ai_input").show_separator_line(false).show_inside(ui, |ui| {
            ui.add_space(4.0);
            if let Some(job) = &self.ai.job {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("{} ({} s)", t("Working..."), job.started.elapsed().as_secs()));
                    if ui.button(t("Stop")).clicked() {
                        job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                });
            }
            let r = ui.add(
                egui::TextEdit::multiline(&mut self.ai.input)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text(t("e.g. Draw a class diagram of a library: Book, Member, Loan")),
            );
            let send_key = r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.command);
            ui.horizontal(|ui| {
                let can = self.ai.job.is_none() && !self.ai.input.trim().is_empty();
                if ui.add_enabled(can, egui::Button::new(t("Send"))).clicked() || (send_key && can) {
                    self.ai_send();
                }
                ui.weak(t("Ctrl+Enter sends · Undo takes a change back"));
            });
            ui.add_space(4.0);
        });
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            if self.ai.chat.is_empty() {
                ui.weak(t("Ask the AI agent to draw a new diagram or to change the open one, in your own words. It sees the open diagram."));
            }
            for (user, text) in &self.ai.chat {
                let (who, color) = if *user { (t("You"), ui.visuals().hyperlink_color) } else { ("AI".to_string(), ui.visuals().strong_text_color()) };
                ui.label(egui::RichText::new(who).small().strong().color(color));
                ui.add(egui::Label::new(text.as_str()).selectable(true).wrap());
                ui.add_space(6.0);
            }
        });
    }
}
