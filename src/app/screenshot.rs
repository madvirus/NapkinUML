//! The --screenshot mode: drawing frames and saving the window as PNG.

use super::*;

/// NAPKINUML_PLAY=<steps> in a --screenshot run feeds scripted input to the app, as if the
/// person did it, without touching the real mouse or keyboard. Steps are separated by `;`:
/// `click x y`, `dclick x y`, `rclick x y`, `drag x0 y0 x1 y1` (window pixels, as in the pictures), `key <Name>`
/// (an egui key name: ArrowDown, Enter, F2, Escape...), `type <text>`, `wait <frames>` and
/// `shot <name>` (saves <picture>-<name>.png beside the --screenshot picture). Each step
/// waits 12 frames after the one before; the --screenshot picture comes after the last.
pub(super) struct Player {
    steps: Vec<String>,
    next: usize,
    wait: u32,
    /// Events still to give, one frame's worth per entry.
    queue: std::collections::VecDeque<Vec<egui::Event>>,
    pub(super) done_frame: Option<u32>,
}

impl Player {
    pub(super) fn new(script: &str) -> Self {
        let steps = script.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        Player { steps, next: 0, wait: 40, queue: Default::default(), done_frame: None }
    }
}

impl UmlApp {
    pub(super) fn play_input(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let Some(frames) = self.screenshot.as_ref().map(|s| s.1) else { return };
        let shot_base = self.screenshot.as_ref().map(|s| s.0.clone()).unwrap_or_default();
        let Some(pl) = self.player.as_mut() else { return };
        if let Some(ev) = pl.queue.pop_front() {
            raw.events.extend(ev);
            return;
        }
        if pl.wait > 0 {
            pl.wait -= 1;
            return;
        }
        if pl.next >= pl.steps.len() {
            if pl.done_frame.is_none() {
                pl.done_frame = Some(frames);
            }
            return;
        }
        let step = pl.steps[pl.next].clone();
        pl.next += 1;
        pl.wait = 12;
        let ppp = ctx.pixels_per_point();
        let mut parts = step.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("").trim().to_string();
        let nums: Vec<f32> = rest.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        let at = Pos2::new(nums.first().copied().unwrap_or(0.0) / ppp, nums.get(1).copied().unwrap_or(0.0) / ppp);
        let m = Modifiers::NONE;
        let button = |b: PointerButton, pressed: bool| egui::Event::PointerButton { pos: at, button: b, pressed, modifiers: m };
        match cmd {
            "click" | "rclick" => {
                let b = if cmd == "rclick" { PointerButton::Secondary } else { PointerButton::Primary };
                pl.queue.push_back(vec![egui::Event::PointerMoved(at)]);
                pl.queue.push_back(vec![button(b, true)]);
                pl.queue.push_back(vec![button(b, false)]);
            }
            "dclick" => {
                pl.queue.push_back(vec![egui::Event::PointerMoved(at)]);
                for _ in 0..2 {
                    pl.queue.push_back(vec![button(PointerButton::Primary, true)]);
                    pl.queue.push_back(vec![button(PointerButton::Primary, false)]);
                }
            }
            "drag" => {
                let to = Pos2::new(nums.get(2).copied().unwrap_or(0.0) / ppp, nums.get(3).copied().unwrap_or(0.0) / ppp);
                pl.queue.push_back(vec![egui::Event::PointerMoved(at)]);
                pl.queue.push_back(vec![button(PointerButton::Primary, true)]);
                for k in 1..=8 {
                    pl.queue.push_back(vec![egui::Event::PointerMoved(at + (to - at) * (k as f32 / 8.0))]);
                }
                pl.queue.push_back(vec![egui::PointerButton::Primary].iter().map(|b| egui::Event::PointerButton { pos: to, button: *b, pressed: false, modifiers: m }).collect());
            }
            "key" => {
                if let Some(key) = Key::from_name(&rest) {
                    let ev = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: m };
                    pl.queue.push_back(vec![ev(true)]);
                    pl.queue.push_back(vec![ev(false)]);
                }
            }
            "type" => pl.queue.push_back(vec![egui::Event::Text(rest.clone())]),
            "wait" => pl.wait = nums.first().map(|n| *n as u32).unwrap_or(12),
            "shot" => {
                let stem = shot_base.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let path = shot_base.with_file_name(format!("{stem}-{rest}.png"));
                self.shot = Some(Shot::Step(path));
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            _ => {}
        }
        ctx.request_repaint();
    }

    pub(super) fn handle_screenshots(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            if let egui::Event::Screenshot { image, .. } = e {
                match self.shot.take() {
                    Some(Shot::Check(path)) => {
                        save_png(&image, &path);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Some(Shot::Step(path)) => {
                        save_png(&image, &path);
                    }
                    None => {}
                }
            }
        }
        // the --screenshot option: draw about 120 frames, then take the picture
        // for checking multi-diagram export: NAPKINUML_EXPORT_ALL=<folder> exports every diagram
        // there a few frames into a --screenshot run
        if let Some((_, 10)) = self.screenshot.as_ref() {
            // for checking Copy as Image: NAPKINUML_COPY_IMAGE=1 copies (the selection, or the diagram)
            if std::env::var("NAPKINUML_COPY_IMAGE").is_ok() {
                self.copy_as_image();
            }
            // for checking Split Activation: NAPKINUML_SPLIT_AT=<x>,<y> (diagram units) right-clicks there
            // and picks Split Activation (or Join Activations when on a split point)
            // for checking activation handles: NAPKINUML_BAR_EDGE=<x>,<y>[,top|bottom,<new y>] selects the
            // bar there and (optionally) moves its top or bottom edge, as dragging its handle does
            if let Ok(v) = std::env::var("NAPKINUML_BAR_EDGE") {
                let parts: Vec<&str> = v.split(',').collect();
                let num = |i: usize| parts.get(i).and_then(|p| p.parse::<f32>().ok());
                let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                if let (Some(d), Some(x), Some(y)) = (self.diag().cloned(), num(0), num(1)) {
                    if let Some(b) = bar_at(&d, &opts, 1.0, (x, y)) {
                        self.clear_selection();
                        self.sel_bar = Some((b.lifeline, b.depth, (b.y0 + b.y1) / 2.0));
                        if let Some(ny) = num(3) {
                            let top = parts.get(2) == Some(&"top");
                            self.checkpoint();
                            if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(b.lifeline)) {
                                stretch_activation(s, b.depth, top, b.y0, b.y1, ny);
                            }
                            self.sel_bar = Some((b.lifeline, b.depth, if top { b.y1 - 1.0 } else { b.y0 + 1.0 }));
                        }
                    }
                }
            }
            // for checking alt sections: NAPKINUML_ALT=<cmd>;<cmd>... on the first alt fragment, each
            // cmd add, remove[@<y>] or move:<line>:<y> (y in diagram units, as the drag and menu do)
            if let Ok(v) = std::env::var("NAPKINUML_ALT") {
                let id = self.diag().and_then(|d| d.shapes.iter().find(|s| s.kind == ShapeKind::AltFragment).map(|s| (s.id, s.y)));
                if let Some((id, top)) = id {
                    for cmd in v.split(';') {
                        let parts: Vec<&str> = cmd.split(':').collect();
                        if cmd == "add" {
                            self.add_alt_section(id);
                        } else if let Some(rest) = cmd.strip_prefix("remove") {
                            let at = rest.strip_prefix('@').and_then(|y| y.parse::<f32>().ok()).map(|y| y - top);
                            self.remove_alt_section(id, at);
                        } else if let (Some(&"move"), Some(i), Some(y)) = (parts.first(), parts.get(1).and_then(|p| p.parse::<usize>().ok()), parts.get(2).and_then(|p| p.parse::<f32>().ok())) {
                            self.checkpoint();
                            if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
                                set_divider(s, i, y);
                            }
                        }
                    }
                    self.clear_selection();
                    self.sel_shapes.push(id);
                }
            }
            // for checking swimlanes: NAPKINUML_LANE=<cmd>;... on the first swimlane, each cmd
            // add@<along>, remove@<along>, edge:<i>:<along> or rename:<i>:<text> (along = distance
            // along the swimlane in diagram units, as the menu and the drag use)
            if let Ok(v) = std::env::var("NAPKINUML_LANE") {
                let lane = self.diag().and_then(|d| d.shapes.iter().find(|s| s.kind.is_swimlane()).map(|s| s.id));
                if let Some(id) = lane {
                    for cmd in v.split(';') {
                        let idx_at = |along: &str| self.diag().and_then(|d| d.shape(id)).map(|s| s.partition_at(along.parse().unwrap_or(0.0))).unwrap_or(0);
                        if let Some(a) = cmd.strip_prefix("add@") {
                            let i = idx_at(a);
                            self.add_partition(id, i);
                        } else if let Some(a) = cmd.strip_prefix("remove@") {
                            let i = idx_at(a);
                            self.remove_partition(id, i);
                        } else {
                            let parts: Vec<&str> = cmd.splitn(3, ':').collect();
                            match parts.as_slice() {
                                ["edge", i, a] => {
                                    self.checkpoint();
                                    if let (Ok(i), Ok(a), Some(s)) = (i.parse::<usize>(), a.parse::<f32>(), self.diag_mut().and_then(|dm| dm.shape_mut(id))) {
                                        s.set_partition_edge(i, a);
                                    }
                                }
                                ["rename", i, t] => {
                                    if let Ok(i) = i.parse::<usize>() {
                                        self.editing = Some(InlineEdit { target: EditTarget::Shape(id), text: t.to_string(), focus: false, line: None, new_line: false, part: Some(i), select_all: false, guard: None });
                                        self.commit_edit();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    self.clear_selection();
                    self.sel_shapes.push(id);
                }
            }
            // for checking end labels: NAPKINUML_END_LABELS=<line index>,<source>,<target>,<sdx>,<sdy>,<tdx>,<tdy>
            if let Ok(v) = std::env::var("NAPKINUML_END_LABELS") {
                let p: Vec<&str> = v.split(',').collect();
                let num = |i: usize| p.get(i).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                if let (Some(i), Some(dm)) = (p.first().and_then(|s| s.parse::<usize>().ok()), self.diag_mut()) {
                    if let Some(c) = dm.connectors.get_mut(i) {
                        c.from_label = p.get(1).unwrap_or(&"").to_string();
                        c.to_label = p.get(2).unwrap_or(&"").to_string();
                        c.from_label_offset = (num(3), num(4));
                        c.to_label_offset = (num(5), num(6));
                    }
                }
            }
            if let Ok(v) = std::env::var("NAPKINUML_SPLIT_AT") {
                let n: Vec<f32> = v.split(',').filter_map(|p| p.parse().ok()).collect();
                let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                if let (Some(d), [x, y]) = (self.diag().cloned(), n.as_slice()) {
                    match seq_menu_target(&d, &opts, 1.0, (*x, *y)) {
                        Some((lid, _, _, Some(ys))) => self.join_activations(lid, ys),
                        Some((lid, yy, true, None)) => self.split_activation(lid, yy),
                        other => self.status = format!("split test: {other:?}"),
                    }
                }
            }
            // for checking drop-to-create: NAPKINUML_DROP_MESSAGE=<message|async|create>,<x>,<y> drops a
            // message from the first lifeline at x (diagram units), pressed at height y
            if let Ok(v) = std::env::var("NAPKINUML_DROP_MESSAGE") {
                let parts: Vec<&str> = v.split(',').collect();
                let kind = match parts.first().copied() { Some("create") => ConnKind::CreateMessage, Some("async") => ConnKind::AsyncMessage, _ => ConnKind::Message };
                let num = |i: usize| parts.get(i).and_then(|p| p.parse::<f32>().ok()).unwrap_or(0.0);
                if let Some(first) = self.diag().and_then(|d| d.shapes.iter().find(|s| s.kind.is_lifeline()).map(|s| s.id)) {
                    self.message_to_new_lifeline(kind, first, num(1), num(2));
                    self.editing = None;
                }
            }
            // for checking the name editor: NAPKINUML_EDIT_SHAPE=<name> opens it on the shape so named
            if let Ok(name) = std::env::var("NAPKINUML_EDIT_SHAPE") {
                if let Some(id) = self.diag().and_then(|d| d.shapes.iter().find(|s| s.name == name).map(|s| s.id)) {
                    self.clear_selection();
                    self.sel_shapes.push(id);
                    self.start_edit(Some(EditTarget::Shape(id)));
                }
            }
            // for checking placement: NAPKINUML_CREATE_SHAPE=<kind>,<x>,<y> creates a shape as a click
            // at that diagram point with the shape tool does (its name editor opens)
            if let Ok(v) = std::env::var("NAPKINUML_CREATE_SHAPE") {
                let parts: Vec<&str> = v.split(',').collect();
                let num = |i: usize| parts.get(i).and_then(|p| p.parse::<f32>().ok()).unwrap_or(0.0);
                if let Ok(kind) = serde_json::from_str::<ShapeKind>(&format!("\"{}\"", parts.first().unwrap_or(&""))) {
                    self.create_shape(kind, num(1), num(2));
                }
            }
            // for checking the Connect and Create menu: NAPKINUML_PICK=<conn kind>,<shape name>,<x>,<y>
            // opens it as releasing that line from the named shape at diagram point x,y does
            if let Ok(v) = std::env::var("NAPKINUML_PICK") {
                let parts: Vec<&str> = v.split(',').collect();
                let num = |i: usize| parts.get(i).and_then(|p| p.parse::<f32>().ok()).unwrap_or(0.0);
                let src = parts.get(1).and_then(|n| self.diag().and_then(|d| d.shapes.iter().find(|s| s.name == *n).map(|s| (s.id, s.kind, d.kind))));
                if let (Ok(ck), Some((id, sk, dk))) = (serde_json::from_str::<ConnKind>(&format!("\"{}\"", parts.first().unwrap_or(&""))), src) {
                    if let Some(default) = pick_default(dk, ck, sk) {
                        let sel = pick_choices(dk, ck, sk).iter().position(|(k, _)| *k == default).unwrap_or(0);
                        self.conn_pick = Some(ConnPick { conn: ck, from: id, at: (num(2), num(3)), sel });
                    }
                }
            }
            // for checking associations to empty space: NAPKINUML_CONN_NEW=<conn kind>,<shape name>,<x>,<y>
            // does what dropping that line from the named shape at diagram point x,y does
            if let Ok(v) = std::env::var("NAPKINUML_CONN_NEW") {
                let parts: Vec<&str> = v.split(',').collect();
                let num = |i: usize| parts.get(i).and_then(|p| p.parse::<f32>().ok()).unwrap_or(0.0);
                let src = parts.get(1).and_then(|n| self.diag().and_then(|d| d.shapes.iter().find(|s| s.name == *n).map(|s| (s.id, s.kind))));
                if let (Ok(ck), Some((id, sk))) = (serde_json::from_str::<ConnKind>(&format!("\"{}\"", parts.first().unwrap_or(&""))), src) {
                    self.conn_to_new_shape(ck, id, sk, num(2), num(3));
                }
            }
            // for checking self messages: NAPKINUML_SELF_MESSAGE=<message|async>,<lifeline number>,<y>;...
            // adds a self message on that lifeline (1 = first) at height y, as clicking it with the tool does
            if let Ok(v) = std::env::var("NAPKINUML_SELF_MESSAGE") {
                for item in v.split(';') {
                    let parts: Vec<&str> = item.split(',').collect();
                    let kind = if parts.first() == Some(&"async") { ConnKind::AsyncMessage } else { ConnKind::Message };
                    let n = parts.get(1).and_then(|p| p.parse::<usize>().ok()).unwrap_or(1).max(1);
                    let y = parts.get(2).and_then(|p| p.parse::<f32>().ok()).unwrap_or(0.0);
                    if let Some(id) = self.diag().and_then(|d| d.shapes.iter().filter(|s| s.kind.is_lifeline()).nth(n - 1).map(|s| s.id)) {
                        self.create_connector(kind, id, id, y);
                    }
                }
            }
            // for checking printing: NAPKINUML_PRINT_PDF=<file.pdf> prints the first three
            // diagrams to that PDF (paper from NAPKINUML_PRINT_PAPER, A4 by default) and saves
            // the page pictures beside it as PNG
            if let Ok(pdf) = std::env::var("NAPKINUML_PRINT_PDF") {
                let ids: Vec<u64> = self.project.as_ref().map(|p| p.diagrams.iter().take(3).map(|d| d.id).collect()).unwrap_or_default();
                let paper = crate::print::Paper::from_id(&std::env::var("NAPKINUML_PRINT_PAPER").unwrap_or_default());
                self.start_print(ids, PrintTo::Pdf(PathBuf::from(pdf)), paper, crate::print::Orient::Auto);
                // NAPKINUML_JOB_HOLD=1 keeps the job at its first step (to photograph the progress window)
                if std::env::var("NAPKINUML_JOB_HOLD").is_ok() {
                    self.jobs.hold();
                }
            }
            if let Ok(dir) = std::env::var("NAPKINUML_EXPORT_ALL") {
                let ids: Vec<u64> = self.project.as_ref().map(|p| p.diagrams.iter().map(|d| d.id).collect()).unwrap_or_default();
                // NAPKINUML_EXPORT_FORMAT=png|jpg|webp picks the format
                let format = ImgFormat::from_id(&std::env::var("NAPKINUML_EXPORT_FORMAT").unwrap_or_default());
                self.start_batch_export(ids, PathBuf::from(dir), format);
            }
        }
        if let Some((path, frames)) = self.screenshot.as_mut() {
            *frames += 1;
            // NAPKINUML_SHOT_FRAME=<n> takes the picture at another frame (default 120)
            let mut at = std::env::var("NAPKINUML_SHOT_FRAME").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(120);
            // with NAPKINUML_PLAY the last picture comes 20 frames after the last step
            if let Some(pl) = &self.player {
                at = pl.done_frame.map(|f| f + 20).unwrap_or(u32::MAX);
            }
            if *frames == at && self.shot.is_none() {
                self.shot = Some(Shot::Check(path.clone()));
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            if *frames > 900 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }

    }
}
