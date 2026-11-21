//! Checking for, downloading and installing updates.

use super::*;

/// Where the update offered at start stands.
pub(super) enum UpdateState {
    Idle,
    Checking(std::sync::mpsc::Receiver<Option<crate::update::Release>>),
    Available(crate::update::Release),
    Downloading(crate::update::Job, crate::update::Release),
    /// Installed: the program to start for the new version.
    Ready(PathBuf, String),
    /// Updating failed: the message and the release page.
    Failed(String, String),
}

impl UmlApp {
    /// Pick up the background update check and download.
    pub(super) fn update_poll(&mut self) {
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
    pub(super) fn update_dialogs(&mut self, ctx: &egui::Context) {
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
}
