//! Sequence diagram layout: activation bars, message numbers and message end points.
//!
//! Activations are worked out from the messages (estimated rules, see MECHANICS.md):
//! * a call (synchronous message) starts an activation on the receiver;
//! * a return message ends the sender's activation, and any nested activations above it,
//!   down to the one the returning-to lifeline started;
//! * asynchronous messages give the receiver a short activation; a create message gives the
//!   lifeline it creates none (its next incoming message starts one);
//! * a lifeline that sends without being active gets an activation starting just before;
//! * when a caller sends again from the activation that made a call that got no return
//!   message, that call counts as answered: the activation it opened ends there;
//! * an activation still open at the end lasts until shortly after the lifeline's last message;
//! * a split the user put on a lifeline (right-click an activation > Split Activation) ends the
//!   activations open there, for when return messages are left out;
//! * stretches the user dragged out (an activation's top or bottom handle) are added, and
//!   activations of one lifeline at the same depth that overlap or touch become one.

use crate::model::{ConnKind, Connector, Diagram, DiagramKind, ProjectOptions, Shape, ShapeKind};
use std::collections::HashMap;

/// Half width of an activation bar and the sideways step of a nested one.
pub const BAR_HALF: f32 = 5.0;
pub const BAR_STEP: f32 = 6.0;
pub const SELF_DROP: f32 = 22.0;
const SHORT_BAR: f32 = 24.0;

#[derive(Clone, Debug)]
pub struct Bar {
    pub lifeline: u64,
    pub y0: f32,
    pub y1: f32,
    pub depth: usize,
}

#[derive(Default)]
pub struct SeqLayout {
    pub bars: Vec<Bar>,
    /// Message number text by connector id.
    pub numbers: HashMap<u64, String>,
    /// Message paths (diagram units) ending at the activation bars.
    pub paths: HashMap<u64, Vec<(f32, f32)>>,
}

struct Building {
    lifeline: u64,
    y0: f32,
    y1: Option<f32>,
    depth: usize,
    started_by: Option<u64>,
    caller: Option<u64>,
    children: u32,
    /// Opened by a (synchronous) self message.
    self_call: bool,
    /// The lowest message sent from this activation (or where it starts).
    last_use: f32,
}

impl Building {
    /// Where an open activation would end if nothing more were sent from it.
    fn natural_end(&self, opts: &ProjectOptions) -> f32 {
        if opts.auto_extend_activations { (self.last_use + 12.0).max(self.y0 + 30.0) } else { self.y0 + 30.0 }
    }
}

fn center_x(s: &Shape) -> f32 {
    s.x + s.w / 2.0
}

/// How far an outer activation runs below a nested one that would end lower.
const NEST_PAD: f32 = 6.0;

/// Activations of a lifeline at the same depth that overlap or touch become one.
fn merge_bars(mut done: Vec<Bar>) -> Vec<Bar> {
    done.sort_by(|a, b| (a.lifeline, a.depth).cmp(&(b.lifeline, b.depth)).then(a.y0.partial_cmp(&b.y0).unwrap_or(std::cmp::Ordering::Equal)));
    let mut merged: Vec<Bar> = Vec::with_capacity(done.len());
    for b in done {
        if let Some(l) = merged.last_mut() {
            if l.lifeline == b.lifeline && l.depth == b.depth && b.y0 <= l.y1 + 0.5 {
                l.y1 = l.y1.max(b.y1);
                continue;
            }
        }
        merged.push(b);
    }
    merged
}

pub fn layout(d: &Diagram, opts: &ProjectOptions) -> SeqLayout {
    let mut out = SeqLayout::default();
    if d.kind != DiagramKind::Sequence {
        return out;
    }
    let mut msgs: Vec<&Connector> = d
        .connectors
        .iter()
        .filter(|c| c.kind.is_message())
        .filter(|c| matches!((d.shape(c.from), d.shape(c.to)), (Some(a), Some(b)) if a.kind.is_lifeline() && b.kind.is_lifeline()))
        .collect();
    msgs.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));

    let mut bars: Vec<Building> = vec![];
    let mut stacks: HashMap<u64, Vec<usize>> = HashMap::new();
    let mut top_counter = 0u32;
    let mut nested: HashMap<u64, String> = HashMap::new();
    // the user's split points, in order of height
    let mut splits: Vec<(f32, u64)> = d.shapes.iter().filter(|s| s.kind.is_lifeline()).flat_map(|s| s.splits.iter().map(move |y| (*y, s.id))).collect();
    splits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut next_split = 0;

    for (i, m) in msgs.iter().enumerate() {
        let y = m.y;
        // splits above this message end that lifeline's open activations
        while next_split < splits.len() && splits[next_split].0 < y {
            let (ys, lid) = splits[next_split];
            if let Some(stack) = stacks.get_mut(&lid) {
                stack.retain(|&b| {
                    if bars[b].y0 < ys {
                        if bars[b].y1.is_none() {
                            bars[b].y1 = Some(ys);
                        }
                        false
                    } else {
                        true
                    }
                });
            }
            next_split += 1;
        }
        // a self message's activation only takes messages that start on it: one sent or received
        // below where it would end (and not on a stretch the user dragged out at its depth)
        // finds it already closed there, so the message belongs to the activation around it
        for lid in [m.from, m.to] {
            let Some(stack) = stacks.get_mut(&lid) else { continue };
            while let Some(&top) = stack.last() {
                let b = &bars[top];
                let end = b.natural_end(opts);
                let stretched = d.shape(lid).map(|s| s.act_spans.iter().any(|sp| sp[2].max(0.0) as usize == b.depth && sp[0].min(sp[1]) <= y && y <= sp[0].max(sp[1]) + 0.5)).unwrap_or(false);
                if b.self_call && b.y1.is_none() && y > end + 2.0 && !stretched {
                    bars[top].y1 = Some(end);
                    stack.pop();
                } else {
                    break;
                }
            }
        }
        // implicit return: a lifeline acting again from the activation that made a call has had
        // that call answered, so the activations the call opened (and what they opened in
        // turn) end here instead of stretching on to later messages
        let since = stacks.get(&m.from).and_then(|s| s.last()).map(|&b| bars[b].y0).unwrap_or(f32::NEG_INFINITY);
        let mut closing: Vec<(u64, f32)> = vec![(m.from, since)];
        while let Some((caller, since)) = closing.pop() {
            for (lid, stack) in stacks.iter_mut() {
                if *lid == m.from {
                    continue;
                }
                let Some(pos) = stack.iter().position(|&b| bars[b].caller == Some(caller) && bars[b].lifeline != caller && bars[b].y0 > since) else { continue };
                for b in stack.drain(pos..).collect::<Vec<_>>() {
                    if bars[b].y1.is_none() {
                        let (lifeline, y0) = (bars[b].lifeline, bars[b].y0);
                        let last = msgs
                            .iter()
                            .filter(|o| (o.from == lifeline || o.to == lifeline) && o.y >= y0 && o.y < y)
                            .map(|o| o.y + if o.from == o.to { SELF_DROP } else { 0.0 })
                            .fold(y0, f32::max);
                        let natural = if opts.auto_extend_activations { (last + 12.0).max(y0 + 30.0) } else { y0 + 30.0 };
                        bars[b].y1 = Some(natural.min(y - 4.0).max(y0 + 10.0));
                    }
                    closing.push((bars[b].lifeline, bars[b].y0));
                }
            }
        }
        // the sender is active while it sends
        if stacks.get(&m.from).map(|s| s.is_empty()).unwrap_or(true) {
            let depth = 0;
            bars.push(Building { lifeline: m.from, y0: y - 6.0, y1: None, depth, started_by: None, caller: None, children: 0, self_call: false, last_use: y - 6.0 });
            stacks.entry(m.from).or_default().push(bars.len() - 1);
        }
        // a return to another lifeline first closes activations the sender started on itself
        if m.kind == ConnKind::ReturnMessage && m.from != m.to {
            let stack = stacks.entry(m.from).or_default();
            while stack.len() > 1 {
                let top = *stack.last().unwrap();
                if bars[top].caller == Some(m.from) {
                    bars[top].y1 = Some(y);
                    stack.pop();
                } else {
                    break;
                }
            }
        }
        let sender_bar = *stacks[&m.from].last().unwrap();
        bars[sender_bar].last_use = bars[sender_bar].last_use.max(y + if m.from == m.to { SELF_DROP } else { 0.0 });
        // numbers: simple 1, 2, 3 ... or nested 1, 1.1, 1.2, 1.2.1 ...
        let nested_no = match bars[sender_bar].started_by {
            Some(s) => {
                bars[sender_bar].children += 1;
                format!("{}.{}", nested.get(&s).cloned().unwrap_or_default(), bars[sender_bar].children)
            }
            None => {
                top_counter += 1;
                top_counter.to_string()
            }
        };
        nested.insert(m.id, nested_no.clone());
        // the diagram's own numbering settings win over the project's
        let auto = if d.numbering(opts) == crate::model::SEQ_SINGLE_LEVEL { (i + 1).to_string() } else { nested_no };
        // Manual handling: a number typed on the message wins (sequenceNumberHandling MANUAL)
        let text = if d.manual(opts) && !m.seq_no.trim().is_empty() { m.seq_no.trim().to_string() } else { auto };
        if d.numbers_messages(opts) {
            out.numbers.insert(m.id, text);
        }

        match m.kind {
            ConnKind::ReturnMessage => {
                let stack = stacks.entry(m.from).or_default();
                if m.from == m.to {
                    if let Some(b) = stack.pop() {
                        bars[b].y1 = Some(y);
                    }
                } else {
                    // close nested activations down to the one the caller started
                    while let Some(b) = stack.pop() {
                        bars[b].y1 = Some(y);
                        if bars[b].caller == Some(m.to) || bars[b].caller.is_none() {
                            break;
                        }
                    }
                }
            }
            ConnKind::Message => {
                let y0 = if m.from == m.to { y + SELF_DROP - 6.0 } else { y };
                let depth = stacks.get(&m.to).map(|s| s.len()).unwrap_or(0);
                bars.push(Building { lifeline: m.to, y0, y1: None, depth, started_by: Some(m.id), caller: Some(m.from), children: 0, self_call: m.from == m.to, last_use: y0 });
                stacks.entry(m.to).or_default().push(bars.len() - 1);
            }
            ConnKind::CreateMessage if m.from != m.to => {
                // a create message only makes the lifeline: no activation on it, one comes with
                // the next message it receives
            }
            _ => {
                // asynchronous messages: a short activation, not waiting for a return
                let y0 = if m.from == m.to { y + SELF_DROP - 6.0 } else { y };
                let depth = stacks.get(&m.to).map(|s| s.len()).unwrap_or(0);
                bars.push(Building { lifeline: m.to, y0, y1: Some(y0 + SHORT_BAR), depth, started_by: Some(m.id), caller: Some(m.from), children: 0, self_call: false, last_use: y0 });
            }
        }
    }
    // activations still open end shortly after the lifeline's last message
    for b in bars.iter_mut() {
        if b.y1.is_none() {
            let last = msgs.iter().filter(|m| (m.from == b.lifeline || m.to == b.lifeline) && m.y >= b.y0).map(|m| m.y + if m.from == m.to { SELF_DROP } else { 0.0 }).fold(b.y0, f32::max);
            // autoExtendActivations (default on): an open activation stretches to the lifeline's last message
            let end = if opts.auto_extend_activations { (last + 12.0).max(b.y0 + 30.0) } else { b.y0 + 30.0 };
            // a split below the last message still cuts it there
            let cut = splits.iter().filter(|(ys, l)| *l == b.lifeline && *ys > b.y0 && *ys < end).map(|(ys, _)| *ys).fold(end, f32::min);
            b.y1 = Some(cut);
        }
    }
    let mut done: Vec<Bar> = bars.into_iter().map(|b| Bar { lifeline: b.lifeline, y0: b.y0, y1: b.y1.unwrap_or(b.y0 + 30.0), depth: b.depth }).collect();
    for s in d.shapes.iter().filter(|s| s.kind.is_lifeline()) {
        for sp in &s.act_spans {
            done.push(Bar { lifeline: s.id, y0: sp[0].min(sp[1]), y1: sp[0].max(sp[1]), depth: sp[2].max(0.0) as usize });
        }
    }
    // overlapping (or touching) activations of a lifeline at the same depth become one; an
    // activation nested in another (a self message's) that runs lower makes the outer one run
    // NEST_PAD below it, deepest first, then they are joined again
    let mut merged = merge_bars(done);
    let deepest = merged.iter().map(|b| b.depth).max().unwrap_or(0);
    for depth in (1..=deepest).rev() {
        let inner: Vec<(u64, f32, f32)> = merged.iter().filter(|b| b.depth == depth).map(|b| (b.lifeline, b.y0, b.y1)).collect();
        for (lid, y0, y1) in inner {
            if let Some(outer) = merged.iter_mut().filter(|o| o.lifeline == lid && o.depth == depth - 1 && o.y0 <= y0 && y0 <= o.y1 + 0.5).last() {
                if outer.y1 < y1 + NEST_PAD {
                    outer.y1 = y1 + NEST_PAD;
                }
            }
        }
    }
    merged = merge_bars(merged);
    out.bars = merged;

    // message ends sit on the outer edge of the top activation at that height
    for m in &msgs {
        let (Some(a), Some(b)) = (d.shape(m.from), d.shape(m.to)) else { continue };
        let edge = |s: &Shape, y: f32, right: bool| -> f32 {
            let cx = center_x(s);
            let top = out.bars.iter().filter(|bar| bar.lifeline == s.id && bar.y0 - 0.5 <= y && y <= bar.y1 + 0.5).map(|bar| bar.depth).max();
            match (opts.show_activations, top) {
                (true, Some(dp)) => cx + dp as f32 * BAR_STEP + if right { BAR_HALF } else { -BAR_HALF },
                _ => cx,
            }
        };
        let y = m.y;
        let path = if m.from == m.to {
            let x1 = edge(a, y, true);
            let x2 = edge(a, y + SELF_DROP, true);
            let out_x = x1.max(x2) + 35.0;
            vec![(x1, y), (out_x, y), (out_x, y + SELF_DROP), (x2, y + SELF_DROP)]
        } else if m.kind == ConnKind::CreateMessage {
            let right = center_x(b) > center_x(a);
            let end = if right { b.x } else { b.x + b.w };
            vec![(edge(a, y, right), y), (end, y)]
        } else {
            let right = center_x(b) > center_x(a);
            vec![(edge(a, y, right), y), (edge(b, y, !right), y)]
        };
        out.paths.insert(m.id, path);
    }
    out
}

/// The point halfway along a path (by length).
pub fn path_midpoint(p: &[(f32, f32)]) -> (f32, f32) {
    if p.is_empty() {
        return (0.0, 0.0);
    }
    let seg = |a: (f32, f32), b: (f32, f32)| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let total: f32 = p.windows(2).map(|w| seg(w[0], w[1])).sum();
    let mut left = total / 2.0;
    for w in p.windows(2) {
        let l = seg(w[0], w[1]);
        if l >= left && l > 0.0 {
            let t = left / l;
            return (w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t);
        }
        left -= l;
    }
    p[p.len() - 1]
}

/// Every connector's drawn path (diagram units): sequence messages on their activation
/// bars, other lines routed by style, and notes attached to a line ending at its middle.
pub fn all_paths(d: &Diagram, opts: &ProjectOptions) -> (SeqLayout, HashMap<u64, Vec<(f32, f32)>>) {
    let lay = layout(d, opts);
    let mut map: HashMap<u64, Vec<(f32, f32)>> = HashMap::new();
    // several free lines between the same two shapes get their own parallel lanes
    let mut groups: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
    for (i, c) in d.connectors.iter().enumerate() {
        if lay.paths.contains_key(&c.id) || c.from == c.to || c.from_anchor.is_some() || c.to_anchor.is_some() || !c.points.is_empty() {
            continue;
        }
        if d.shape(c.from).is_none() || d.shape(c.to).is_none() {
            continue;
        }
        groups.entry((c.from.min(c.to), c.from.max(c.to))).or_default().push(i);
    }
    for (_, idx) in groups.iter().filter(|(_, v)| v.len() > 1) {
        let n = idx.len();
        for (k, &i) in idx.iter().enumerate() {
            let c = &d.connectors[i];
            let (a, b) = (d.shape(c.from).unwrap(), d.shape(c.to).unwrap());
            if let Some((fa, fb)) = crate::render::lane_anchors(a, b, k, n) {
                let mut lane = c.clone();
                lane.from_anchor = Some(fa);
                lane.to_anchor = Some(fb);
                map.insert(c.id, crate::render::conn_path(d.kind, &lane, a, b, opts.connector_style));
            }
        }
    }
    for c in &d.connectors {
        if map.contains_key(&c.id) {
            continue;
        }
        if let Some(p) = lay.paths.get(&c.id) {
            map.insert(c.id, p.clone());
        } else if let (Some(a), Some(b)) = (d.shape(c.from), d.shape(c.to)) {
            map.insert(c.id, crate::render::conn_path(d.kind, c, a, b, opts.connector_style));
        }
    }
    for c in &d.connectors {
        if c.kind == ConnKind::Anchor && d.shape(c.to).is_none() {
            if let (Some(note), Some(target)) = (d.shape(c.from), map.get(&c.to).cloned()) {
                let mid = path_midpoint(&target);
                let first = c.points.first().copied().unwrap_or(mid);
                let mut v = vec![crate::render::boundary_point(note, first)];
                v.extend(c.points.iter().copied());
                v.push(mid);
                map.insert(c.id, v);
            }
        }
    }
    (lay, map)
}

/// The activation bar under a point (diagram units) of a sequence diagram, innermost first.
pub fn bar_at(d: &Diagram, opts: &ProjectOptions, zoom: f32, (x, y): (f32, f32)) -> Option<Bar> {
    if d.kind != DiagramKind::Sequence || !opts.show_activations {
        return None;
    }
    let slack = 4.0 / zoom.max(0.1);
    let lay = layout(d, opts);
    lay.bars
        .iter()
        .filter(|b| y >= b.y0 && y <= b.y1)
        .filter(|b| {
            d.shape(b.lifeline).map(|s| {
                let cx = s.x + s.w / 2.0 + b.depth as f32 * BAR_STEP;
                (x - cx).abs() <= BAR_HALF + slack
            }).unwrap_or(false)
        })
        .max_by_key(|b| b.depth)
        .cloned()
}

/// Move an activation's top or bottom edge to height `ny`. Stretching out adds a stretch
/// (which joins any activation it reaches); pulling the bottom in ends it there with a split;
/// pulling the top back in only takes back stretches (a bar can't start after its message).
pub fn stretch_activation(s: &mut Shape, depth: usize, top: bool, y0: f32, y1: f32, ny: f32) {
    let dep = depth as f32;
    if top {
        let ny = ny.min(y1 - 10.0);
        if ny < y0 {
            s.act_spans.push([ny, y0 + 1.0, dep]);
        } else {
            for sp in s.act_spans.iter_mut().filter(|sp| sp[2] == dep && sp[0] < ny && sp[1] > y0 - 0.5) {
                sp[0] = ny;
            }
        }
    } else {
        let ny = ny.max(y0 + 10.0);
        if ny > y1 {
            s.act_spans.push([y1 - 1.0, ny, dep]);
        } else {
            for sp in s.act_spans.iter_mut().filter(|sp| sp[2] == dep && sp[1] > ny && sp[0] < y1 + 0.5) {
                sp[1] = ny;
            }
            if ny < y1 - 0.5 {
                s.splits.push(ny);
                s.splits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }
    s.act_spans.retain(|sp| sp[1] - sp[0] > 0.5);
}

/// What a right-click at this point (diagram units) touches in a sequence diagram: the
/// lifeline, the height, whether it is on an activation bar, and a split point near it.
pub fn seq_menu_target(d: &Diagram, opts: &ProjectOptions, zoom: f32, (x, y): (f32, f32)) -> Option<(u64, f32, bool, Option<f32>)> {
    if d.kind != DiagramKind::Sequence {
        return None;
    }
    let lay = layout(d, opts);
    let slack = 4.0 / zoom.max(0.1);
    let bar = lay.bars.iter().filter(|b| y >= b.y0 && y <= b.y1).find(|b| {
        d.shape(b.lifeline).map(|s| {
            let cx = s.x + s.w / 2.0 + b.depth as f32 * BAR_STEP;
            opts.show_activations && (x - cx).abs() <= BAR_HALF + slack
        }).unwrap_or(false)
    });
    let line = d.shapes.iter().filter(|s| s.kind.is_lifeline() && y > s.y && y < s.y + s.h).find(|s| (x - (s.x + s.w / 2.0)).abs() <= BAR_HALF + 3.0 * BAR_STEP + slack);
    let lid = bar.map(|b| b.lifeline).or(line.map(|s| s.id))?;
    let near = d.shape(lid).and_then(|s| s.splits.iter().copied().filter(|v| (v - y).abs() <= 12.0 / zoom.max(0.1)).min_by(|a, b| (a - y).abs().partial_cmp(&(b - y).abs()).unwrap_or(std::cmp::Ordering::Equal)));
    Some((lid, y, bar.is_some(), near))
}

/// In a sequence diagram the lifelines and actors along the top move only sideways, so they
/// stay lined up; a lifeline made by a Create message sits at that message instead.
pub fn y_locked(d: &Diagram, s: &Shape) -> bool {
    d.kind == DiagramKind::Sequence && s.kind.is_lifeline() && !is_created(d, s.id)
}

/// The lifeline is the target of a Create message.
pub fn is_created(d: &Diagram, id: u64) -> bool {
    d.connectors.iter().any(|c| c.kind == ConnKind::CreateMessage && c.to == id)
}

/// Height of a lifeline's head box (where a Create message meets it in the middle).
pub fn lifeline_head(kind: ShapeKind) -> f32 {
    if kind == ShapeKind::Lifeline { 40.0 } else { 70.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, NAV_TRUE};

    fn lifeline(id: u64) -> Shape {
        Shape::new(id, ShapeKind::Lifeline, id as f32 * 150.0, 20.0, 100.0, 400.0)
    }

    fn message(id: u64, kind: ConnKind, from: u64, to: u64, y: f32) -> Connector {
        Connector {
            id,
            kind,
            from,
            to,
            name: String::new(),
            from_label: String::new(),
            to_label: String::new(),
            y,
            documentation: String::new(),
            points: vec![],
            arrow_start: false,
            arrow_end: false,
            style: 3,
            nav_from: NAV_TRUE,
            nav_to: NAV_TRUE,
            seq_no: String::new(),
            from_label_offset: (0.0, 0.0),
            to_label_offset: (0.0, 0.0),
            label_offset: (0.0, 0.0),
            from_anchor: None,
            to_anchor: None,
        }
    }

    fn diagram(kind: DiagramKind, shapes: Vec<Shape>, connectors: Vec<Connector>) -> Diagram {
        let mut p = Project::new("t");
        let did = p.add_diagram(kind, None);
        let mut d = p.diagram_mut(did).unwrap().clone();
        d.shapes = shapes;
        d.connectors = connectors;
        d
    }

    #[test]
    fn stretching_an_activation() {
        let mut s = lifeline(1);
        stretch_activation(&mut s, 0, true, 100.0, 200.0, 50.0);
        assert_eq!(s.act_spans, vec![[50.0, 101.0, 0.0]]);

        let mut s = lifeline(1);
        stretch_activation(&mut s, 1, false, 100.0, 200.0, 250.0);
        assert_eq!(s.act_spans, vec![[199.0, 250.0, 1.0]]);

        // pulling the bottom in ends the activation there with a split
        let mut s = lifeline(1);
        stretch_activation(&mut s, 0, false, 100.0, 200.0, 150.0);
        assert!(s.act_spans.is_empty());
        assert_eq!(s.splits, vec![150.0]);

        // never shorter than 10
        let mut s = lifeline(1);
        stretch_activation(&mut s, 0, false, 100.0, 200.0, 0.0);
        assert_eq!(s.splits, vec![110.0]);
    }

    #[test]
    fn created_lifelines_are_not_y_locked() {
        let d = diagram(DiagramKind::Sequence, vec![lifeline(1), lifeline(2), lifeline(3)], vec![message(10, ConnKind::CreateMessage, 1, 2, 150.0), message(11, ConnKind::Message, 1, 3, 200.0)]);
        assert!(!is_created(&d, 1));
        assert!(is_created(&d, 2));
        assert!(!is_created(&d, 3));
        assert!(y_locked(&d, &d.shapes[0]));
        assert!(!y_locked(&d, &d.shapes[1]));
        assert!(y_locked(&d, &d.shapes[2]));
        let c = diagram(DiagramKind::Class, vec![lifeline(1)], vec![]);
        assert!(!y_locked(&c, &c.shapes[0]));
    }

    #[test]
    fn lifeline_head_heights() {
        assert_eq!(lifeline_head(ShapeKind::Lifeline), 40.0);
        assert_eq!(lifeline_head(ShapeKind::ActorLifeline), 70.0);
    }

    #[test]
    fn bar_at_finds_the_activation_under_a_point() {
        let d = diagram(DiagramKind::Sequence, vec![lifeline(1), lifeline(2)], vec![message(10, ConnKind::Message, 1, 2, 150.0)]);
        let opts = ProjectOptions::default();
        let lay = layout(&d, &opts);
        let b = lay.bars.iter().find(|b| b.lifeline == 2).expect("the receiver is activated").clone();
        let s = d.shape(2).unwrap();
        let cx = s.x + s.w / 2.0;
        let mid = (b.y0 + b.y1) / 2.0;
        assert_eq!(bar_at(&d, &opts, 1.0, (cx, mid)).map(|b| b.lifeline), Some(2));
        assert!(bar_at(&d, &opts, 1.0, (cx + 40.0, mid)).is_none());
        let off = ProjectOptions { show_activations: false, ..ProjectOptions::default() };
        assert!(bar_at(&d, &off, 1.0, (cx, mid)).is_none());
        let t = seq_menu_target(&d, &opts, 1.0, (cx, mid)).unwrap();
        assert_eq!((t.0, t.2), (2, true));
    }
}
