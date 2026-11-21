//! The fonts given to egui: which are loaded, and when to look again.

use crate::render;

pub(super) struct FontState {
    /// The fonts given to egui, also used to draw diagrams off screen.
    defs: egui::FontDefinitions,
    loaded: Vec<String>,
    /// Given to egui this frame; usable for drawing from the next one.
    pending: Vec<String>,
    ready: bool,
    /// The fonts in use may have changed: look again next frame.
    check: bool,
}

impl FontState {
    pub(super) fn new() -> Self {
        FontState { defs: egui::FontDefinitions::default(), loaded: vec![], pending: vec![], ready: false, check: true }
    }

    pub(super) fn defs(&self) -> &egui::FontDefinitions {
        &self.defs
    }

    /// Look at the fonts in use again next frame.
    pub(super) fn request_refresh(&mut self) {
        self.check = true;
    }

    /// Fonts given to egui last frame are usable now. True when the fonts in use should be
    /// looked at again.
    pub(super) fn begin_frame(&mut self) -> bool {
        for n in self.pending.drain(..) {
            render::register_family(&n);
        }
        self.check
    }

    /// Load the named fonts (those not installed are skipped); egui can draw with them next frame.
    pub(super) fn load(&mut self, ctx: &egui::Context, names: Vec<String>) {
        self.check = false;
        let mut wanted = self.loaded.clone();
        for n in names {
            if !n.is_empty() && crate::fonts::available().contains(&n.as_str()) && !wanted.contains(&n) {
                wanted.push(n);
            }
        }
        if self.ready && wanted.len() == self.loaded.len() {
            return;
        }
        let (defs, loaded) = crate::fonts::build(&wanted);
        self.defs = defs.clone();
        ctx.set_fonts(defs);
        self.ready = true;
        self.pending = loaded.clone();
        self.loaded = loaded;
        ctx.request_repaint();
    }
}
