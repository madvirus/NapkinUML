//! Mind map structures (as in XMind): the topics below a root topic are placed automatically,
//! and a dragged topic joins the topic it is dropped next to.
//!
//! A root topic (the central topic, or a floating topic with no parent) keeps its structure in
//! `Shape::orient`: 0 Mind Map (sub-topics balanced right and left), 1 Logic Chart (all to the
//! right), 2 Org Chart (downward hierarchy). Only the root stays where it is put; every topic
//! below it is placed from the tree. Siblings keep the order of their current positions (top
//! to bottom, or left to right in an org chart), so moving a topic among its siblings reorders it.

use crate::model::{ConnKind, Diagram, ShapeKind};
use crate::render::{STYLE_ARC, STYLE_BENT, STYLE_CURVE, STYLE_FOLLOW_DIAGRAM, STYLE_OBLIQUE, STYLE_RECTILINEAR, STYLE_ROUND_BENT, STYLE_ROUND_RECTILINEAR};
use std::collections::{HashMap, HashSet};

pub const MIND_MAP: u8 = 0;
pub const LOGIC_CHART: u8 = 1;
pub const ORG_CHART: u8 = 2;
/// The structures a root topic offers, in menu order.
pub const STRUCTURES: [(u8, &str); 3] = [(MIND_MAP, "Mind Map"), (LOGIC_CHART, "Logic Chart"), (ORG_CHART, "Org Chart")];

/// Room between a topic and its sub-topics beside it, and between sibling sub-trees above each
/// other (diagram units).
pub const GAP_X: f32 = 50.0;
pub const GAP_Y: f32 = 14.0;
/// Org chart: room between a topic and the sub-topics below it, and between sibling sub-trees.
pub const GAP_V: f32 = 40.0;
pub const GAP_H: f32 = 20.0;
/// Room between the central topic's main branches (each with all its sub-topics), wider than
/// between other sibling sub-trees so each line of thought stands apart: above each other in a
/// mind map or logic chart, side by side in an org chart.
pub const MAIN_GAP_Y: f32 = 32.0;
pub const MAIN_GAP_H: f32 = 44.0;
/// A mind map's sides are evened out only when one holds more than this many topics more than
/// the other (counting every level below the root), so adding a topic doesn't shuffle the map
/// every time.
pub const REBALANCE_AFTER: usize = 2;
/// The branch styles a topic offers for the branches to its sub-topics, in menu order.
pub const BRANCH_STYLES: [(u8, &str); 7] = [
    (STYLE_ROUND_RECTILINEAR, "Round rectilinear"),
    (STYLE_RECTILINEAR, "Rectilinear"),
    (STYLE_OBLIQUE, "Straight"),
    (STYLE_ARC, "Arc"),
    (STYLE_CURVE, "Curve"),
    (STYLE_BENT, "Bent"),
    (STYLE_ROUND_BENT, "Round bent"),
];
/// A dropped topic further than this (weighted) from every place it could join floats free.
pub const JOIN_REACH: f32 = 200.0;

/// Which way a topic's sub-topics grow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grow {
    Right,
    Left,
    Down,
    /// The root of a mind map: both sides.
    Both,
}

#[derive(Clone, Copy)]
struct R {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl R {
    fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
}

/// The topic tree of a diagram: each topic's parent (the first Branch into it from another
/// topic) and its children in line order.
struct Tree {
    rect: HashMap<u64, R>,
    parent: HashMap<u64, u64>,
    kids: HashMap<u64, Vec<u64>>,
    structure: HashMap<u64, u8>,
    /// How many topics each folded topic keeps folded away.
    folded: HashMap<u64, usize>,
    /// The branch style each topic sets for its own branches (`Shape::branch_style`).
    branch: HashMap<u64, u8>,
}

impl Tree {
    fn new(d: &Diagram) -> Self {
        let mut rect = HashMap::new();
        let mut structure = HashMap::new();
        let mut folded = HashMap::new();
        let mut branch = HashMap::new();
        for s in d.shapes.iter().filter(|s| s.kind.is_topic()) {
            if let Some(b) = s.branch_style {
                branch.insert(s.id, b);
            }
            rect.insert(s.id, R { x: s.x, y: s.y, w: s.w, h: s.h });
            structure.insert(s.id, s.orient.min(ORG_CHART));
            if let Some(f) = &s.fold {
                folded.insert(s.id, f.topics());
            }
        }
        let mut parent = HashMap::new();
        let mut kids: HashMap<u64, Vec<u64>> = HashMap::new();
        for c in &d.connectors {
            if c.kind == ConnKind::Branch && c.from != c.to && rect.contains_key(&c.from) && rect.contains_key(&c.to) && !parent.contains_key(&c.to) {
                parent.insert(c.to, c.from);
                kids.entry(c.from).or_default().push(c.to);
            }
        }
        Tree { rect, parent, kids, structure, folded, branch }
    }

    /// The branch style of the branches leaving topic `id`: its own, else the nearest topic's
    /// above it that sets one, else its structure's (a curve; round right angles in an org chart).
    fn branch_style(&self, id: u64, org: bool) -> u8 {
        let mut seen = HashSet::new();
        let mut at = id;
        loop {
            if let Some(b) = self.branch.get(&at) {
                return *b;
            }
            match self.parent.get(&at) {
                Some(p) if seen.insert(at) => at = *p,
                _ => break,
            }
        }
        if org {
            STYLE_ROUND_RECTILINEAR
        } else {
            STYLE_CURVE
        }
    }

    fn kids(&self, id: u64) -> &[u64] {
        self.kids.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// The root above a topic; None when its parents go round in a loop.
    fn root(&self, id: u64) -> Option<u64> {
        let mut seen = HashSet::new();
        let mut at = id;
        while let Some(&p) = self.parent.get(&at) {
            if !seen.insert(at) {
                return None;
            }
            at = p;
        }
        Some(at)
    }

    fn grow(&self, id: u64) -> Option<Grow> {
        let root = self.root(id)?;
        Some(match self.structure.get(&root).copied().unwrap_or(MIND_MAP) {
            ORG_CHART => Grow::Down,
            LOGIC_CHART => Grow::Right,
            _ if id == root => Grow::Both,
            _ => {
                if self.rect[&id].cx() >= self.rect[&root].cx() {
                    Grow::Right
                } else {
                    Grow::Left
                }
            }
        })
    }

    /// Children ordered by where they are now (`key` of their rectangle), line order on ties.
    fn sorted(&self, list: &[u64], key: impl Fn(&R) -> f32) -> Vec<u64> {
        let mut v: Vec<(usize, u64)> = list.iter().copied().enumerate().collect();
        v.sort_by(|a, b| key(&self.rect[&a.1]).partial_cmp(&key(&self.rect[&b.1])).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
        v.into_iter().map(|(_, id)| id).collect()
    }

    /// How many topics a sub-tree holds: the topic and every level below it, folded ones too
    /// (so folding or unfolding a topic never moves topics to the other side).
    fn size(&self, id: u64) -> usize {
        1 + self.folded.get(&id).copied().unwrap_or(0) + self.kids(id).iter().map(|c| self.size(*c)).sum::<usize>()
    }

    /// Height a topic's sub-tree takes when it grows sideways.
    fn ext_v(&self, id: u64) -> f32 {
        let k = self.kids(id);
        let h = self.rect[&id].h;
        if k.is_empty() {
            return h;
        }
        let sum: f32 = k.iter().map(|c| self.ext_v(*c)).sum::<f32>() + GAP_Y * (k.len() - 1) as f32;
        sum.max(h)
    }

    /// Width a topic's sub-tree takes when it grows downward.
    fn ext_h(&self, id: u64) -> f32 {
        let k = self.kids(id);
        let w = self.rect[&id].w;
        if k.is_empty() {
            return w;
        }
        let sum: f32 = k.iter().map(|c| self.ext_h(*c)).sum::<f32>() + GAP_H * (k.len() - 1) as f32;
        sum.max(w)
    }

    /// Place `list` (children of the topic now at `at`) beside it, stacked and centred on it,
    /// `gap` apart.
    fn place_side(&self, at: R, list: &[u64], right: bool, gap: f32, out: &mut HashMap<u64, R>) {
        if list.is_empty() {
            return;
        }
        let total: f32 = list.iter().map(|c| self.ext_v(*c)).sum::<f32>() + gap * (list.len() - 1) as f32;
        let mut y0 = at.cy() - total / 2.0;
        for &c in list {
            let e = self.ext_v(c);
            let r = self.rect[&c];
            let x = if right { at.x + at.w + GAP_X } else { at.x - GAP_X - r.w };
            let nr = R { x, y: y0 + e / 2.0 - r.h / 2.0, ..r };
            out.insert(c, nr);
            let next = self.sorted(self.kids(c), |r| r.cy());
            self.place_side(nr, &next, right, GAP_Y, out);
            y0 += e + gap;
        }
    }

    /// Place the children of the topic now at `at` below it, side by side and centred under it,
    /// `gap` apart.
    fn place_down(&self, id: u64, at: R, gap: f32, out: &mut HashMap<u64, R>) {
        let list = self.sorted(self.kids(id), |r| r.cx());
        if list.is_empty() {
            return;
        }
        let total: f32 = list.iter().map(|c| self.ext_h(*c)).sum::<f32>() + gap * (list.len() - 1) as f32;
        let mut x0 = at.cx() - total / 2.0;
        for c in list {
            let e = self.ext_h(c);
            let r = self.rect[&c];
            let nr = R { x: x0 + e / 2.0 - r.w / 2.0, y: at.y + at.h + GAP_V, ..r };
            out.insert(c, nr);
            self.place_down(c, nr, GAP_H, out);
            x0 += e + gap;
        }
    }
}

/// The children of a topic, in line order.
pub fn children(d: &Diagram, id: u64) -> Vec<u64> {
    Tree::new(d).kids(id).to_vec()
}

/// Every topic that has a fold knob (a Topic below another topic with sub-topics, or one folded
/// already; never a central topic): the topic, the side its sub-topics grow to (a mind map
/// root's right), and how many topics it has folded away (None while unfolded).
pub fn fold_knobs(d: &Diagram) -> Vec<(u64, Grow, Option<usize>)> {
    if d.kind != crate::model::DiagramKind::MindMap {
        return vec![];
    }
    let t = Tree::new(d);
    d.shapes
        .iter()
        .filter(|s| s.kind == ShapeKind::Topic && (s.fold.is_some() || (t.parent.contains_key(&s.id) && !t.kids(s.id).is_empty())))
        .map(|s| {
            let g = match t.grow(s.id).unwrap_or(Grow::Right) {
                Grow::Both => Grow::Right,
                g => g,
            };
            (s.id, g, s.fold.as_ref().map(|f| f.topics()))
        })
        .collect()
}

/// The topic an arrow key moves the selection to from topic `id` (`dir` is (-1, 0) left,
/// (1, 0) right, (0, -1) up or (0, 1) down):
/// * toward its sub-topics (the way they grow; a mind map root's either side): the sub-topic on
///   that side nearest level with it, or nowhere when it has none (or they are folded);
/// * toward its parent (back the way it grew): the parent;
/// * across (up and down beside it, left and right in an org chart): the nearest sibling that
///   way, else the nearest topic that way anywhere (a cousin, or the far side of the map).
/// Distance that way counts once, distance off to the side twice.
pub fn neighbour(d: &Diagram, id: u64, dir: (i8, i8)) -> Option<u64> {
    let t = Tree::new(d);
    let me = *t.rect.get(&id)?;
    let (dx, dy) = (dir.0 as f32, dir.1 as f32);
    let along = |r: &R| (r.cx() - me.cx()) * dx + (r.cy() - me.cy()) * dy;
    let across = |r: &R| ((r.cx() - me.cx()) * dy).abs() + ((r.cy() - me.cy()) * dx).abs();
    let score = |k: &u64| {
        let r = &t.rect[k];
        along(r) + 2.0 * across(r)
    };
    let best = |list: Vec<u64>, by: &dyn Fn(&u64) -> f32| list.into_iter().min_by(|a, b| by(a).partial_cmp(&by(b)).unwrap_or(std::cmp::Ordering::Equal));
    let grow = t.grow(id)?;
    let toward_kids = match grow {
        Grow::Down => dir == (0, 1),
        Grow::Right => dir == (1, 0),
        Grow::Left => dir == (-1, 0),
        Grow::Both => dir.1 == 0,
    };
    if toward_kids {
        let kids: Vec<u64> = t.kids(id).iter().copied().filter(|k| along(&t.rect[k]) > 0.0).collect();
        return best(kids, &|k| across(&t.rect[k]));
    }
    if let Some(&p) = t.parent.get(&id) {
        let back = if grow == Grow::Down { dir == (0, -1) } else { dir.1 == 0 && along(&t.rect[&p]) > 0.0 };
        if back {
            return Some(p);
        }
        let sibs: Vec<u64> = t.kids(p).iter().copied().filter(|s| *s != id && along(&t.rect[s]) > 0.0).collect();
        if let Some(s) = best(sibs, &score) {
            return Some(s);
        }
    }
    let any: Vec<u64> = t.rect.keys().copied().filter(|k| *k != id && along(&t.rect[k]) > 0.0).collect();
    best(any, &score)
}

/// Which way a topic's sub-topics grow (None when its parents loop).
pub fn grow(d: &Diagram, id: u64) -> Option<Grow> {
    Tree::new(d).grow(id)
}

/// Place every topic below each root topic by its root's structure, and route its branches:
/// level S curves sideways, right-angle elbows in an org chart. Root topics stay put.
/// A mind map root keeps its sub-topics in clockwise order: down the right side from the top,
/// then up the left side from the bottom. Each stays on the side it is on. A side's weight is
/// every topic in its sub-trees, all levels down; only when one side weighs more than
/// `REBALANCE_AFTER` more than the other are they evened out, by moving the boundary at the
/// bottom: the lowest sub-tree on the heavier side goes to the bottom of the other side, again
/// and again while that brings the weights closer (on a tie the right keeps the extra), so the
/// order stays clockwise.
pub fn layout(d: &mut Diagram) {
    layout_keep(d, None);
}

/// `layout`, where evening out a mind map's sides leaves `keep` (a topic just dropped)
/// on the side it was put, moving another topic over instead.
pub fn layout_keep(d: &mut Diagram, keep: Option<u64>) {
    if d.kind != crate::model::DiagramKind::MindMap {
        return;
    }
    let t = Tree::new(d);
    let mut out: HashMap<u64, R> = HashMap::new();
    let mut org: HashSet<u64> = HashSet::new();
    let mut roots: Vec<u64> = t.rect.keys().copied().filter(|id| !t.parent.contains_key(id) && !t.kids(*id).is_empty()).collect();
    roots.sort();
    for root in roots {
        let at = t.rect[&root];
        match t.structure[&root] {
            ORG_CHART => {
                t.place_down(root, at, MAIN_GAP_H, &mut out);
                let mut stack = vec![root];
                while let Some(n) = stack.pop() {
                    org.insert(n);
                    stack.extend_from_slice(t.kids(n));
                }
            }
            LOGIC_CHART => {
                let list = t.sorted(t.kids(root), |r| r.cy());
                t.place_side(at, &list, true, MAIN_GAP_Y, &mut out);
            }
            _ => {
                let (mut right, mut left): (Vec<u64>, Vec<u64>) = t.kids(root).iter().partition(|c| t.rect[*c].cx() >= at.cx());
                right = t.sorted(&right, |r| r.cy());
                left = t.sorted(&left, |r| r.cy());
                // the lowest topic of a side that isn't `keep` (or `keep` when it is alone there)
                let take = |v: &mut Vec<u64>| {
                    let i = v.iter().rposition(|c| Some(*c) != keep).unwrap_or(v.len() - 1);
                    v.remove(i)
                };
                // weigh each side by every topic in its sub-trees, all levels down
                let weight = |v: &[u64]| v.iter().map(|c| t.size(*c) as i64).sum::<i64>();
                let diff = weight(&right) - weight(&left);
                if diff.unsigned_abs() as usize > REBALANCE_AFTER {
                    if diff > 0 {
                        // right heavier: its lowest sub-tree goes over while that brings the sides closer
                        while let Some(i) = right.iter().rposition(|c| Some(*c) != keep).or(right.len().checked_sub(1)) {
                            let d = weight(&right) - weight(&left);
                            if d <= 0 || t.size(right[i]) as i64 >= d {
                                break;
                            }
                            let c = take(&mut right);
                            left.push(c);
                        }
                    } else {
                        // left heavier: the same the other way (a tie leaves the right heavier)
                        while let Some(i) = left.iter().rposition(|c| Some(*c) != keep).or(left.len().checked_sub(1)) {
                            let d = weight(&left) - weight(&right);
                            if d <= 0 || t.size(left[i]) as i64 > d {
                                break;
                            }
                            let c = take(&mut left);
                            right.push(c);
                        }
                    }
                }
                t.place_side(at, &right, true, MAIN_GAP_Y, &mut out);
                t.place_side(at, &left, false, MAIN_GAP_Y, &mut out);
            }
        }
    }
    for s in d.shapes.iter_mut() {
        if let Some(r) = out.get(&s.id) {
            if (s.x - r.x).abs() > 0.01 || (s.y - r.y).abs() > 0.01 {
                s.x = r.x;
                s.y = r.y;
            }
        }
    }
    // the branches of the tree follow its structure, drawn in the branch style their parent topic
    // sets (or inherits): from the parent's side facing the sub-topic to the sub-topic's near side
    let centre: HashMap<u64, f32> = d.shapes.iter().map(|s| (s.id, s.center().0)).collect();
    // an underlined topic's branches meet the line under its name (its bottom), not its middle
    let under: HashSet<u64> = d.shapes.iter().filter(|s| s.underlined()).map(|s| s.id).collect();
    let mut done: HashSet<u64> = HashSet::new();
    for c in d.connectors.iter_mut() {
        if c.kind != ConnKind::Branch || t.parent.get(&c.to) != Some(&c.from) || !done.insert(c.to) {
            continue;
        }
        let down = org.contains(&c.from);
        let right = centre.get(&c.to).copied().unwrap_or(0.0) >= centre.get(&c.from).copied().unwrap_or(0.0);
        let (fa, ta) = if down {
            (Some((0.5, 1.0)), Some((0.5, 0.0)))
        } else {
            let (fy, ty) = (if under.contains(&c.from) { 1.0 } else { 0.5 }, if under.contains(&c.to) { 1.0 } else { 0.5 });
            if right {
                (Some((1.0, fy)), Some((0.0, ty)))
            } else {
                (Some((0.0, fy)), Some((1.0, ty)))
            }
        };
        let (style, fa, ta) = match t.branch_style(c.from, down) {
            // a level S curve sideways (render::branch_curve); downward it keeps its ends
            STYLE_CURVE if !down => (STYLE_FOLLOW_DIAGRAM, None, None),
            STYLE_CURVE => (STYLE_FOLLOW_DIAGRAM, fa, ta),
            s => (s, fa, ta),
        };
        if c.style != style || c.from_anchor != fa || c.to_anchor != ta || !c.points.is_empty() {
            c.style = style;
            c.from_anchor = fa;
            c.to_anchor = ta;
            c.points.clear();
        }
    }
}

/// Where a dragged topic would join: the topic it is dropped on, or else the topic whose
/// sub-topic column it lands in (beside it on the side it grows to, or below it in an org
/// chart), scored by how far the topic is from that column plus 0.6 × how far it is from the
/// span of that topic and its children. None (it floats free) when nothing is within
/// `JOIN_REACH`. Never the topic itself or one of its own sub-topics. Returns the new parent
/// and the way it grows there.
pub fn drop_parent(d: &Diagram, id: u64) -> Option<(u64, Grow)> {
    let t = Tree::new(d);
    let me = *t.rect.get(&id)?;
    let (qx, qy) = (me.cx(), me.cy());
    let mut own = HashSet::new();
    let mut stack = vec![id];
    while let Some(n) = stack.pop() {
        if own.insert(n) {
            stack.extend_from_slice(t.kids(n));
        }
    }
    let mut ids: Vec<u64> = t.rect.keys().copied().filter(|p| !own.contains(p)).collect();
    ids.sort();
    // dropped onto a topic: it joins that one
    if let Some(&p) = ids.iter().rev().find(|p| {
        let r = t.rect[*p];
        qx >= r.x && qx <= r.x + r.w && qy >= r.y && qy <= r.y + r.h
    }) {
        let g = t.grow(p).unwrap_or(Grow::Right);
        let g = if g == Grow::Both { if qx >= t.rect[&p].cx() { Grow::Right } else { Grow::Left } } else { g };
        return Some((p, g));
    }
    let mut best: Option<(f32, u64, Grow)> = None;
    for p in ids {
        let Some(g) = t.grow(p) else { continue };
        let pr = t.rect[&p];
        let (mut top, mut bottom, mut left, mut right) = (pr.y, pr.y + pr.h, pr.x, pr.x + pr.w);
        for k in t.kids(p).iter().filter(|k| !own.contains(k)) {
            let r = t.rect[k];
            top = top.min(r.y);
            bottom = bottom.max(r.y + r.h);
            left = left.min(r.x);
            right = right.max(r.x + r.w);
        }
        let off = |q: f32, lo: f32, hi: f32| if q < lo { lo - q } else if q > hi { q - hi } else { 0.0 };
        let dirs: &[Grow] = match g {
            Grow::Both => &[Grow::Right, Grow::Left],
            Grow::Right => &[Grow::Right],
            Grow::Left => &[Grow::Left],
            Grow::Down => &[Grow::Down],
        };
        for &dir in dirs {
            let score = match dir {
                Grow::Right if qx > pr.x + pr.w * 0.75 => (me.x - (pr.x + pr.w + GAP_X)).abs() + 0.6 * off(qy, top, bottom),
                Grow::Left if qx < pr.x + pr.w * 0.25 => ((me.x + me.w) - (pr.x - GAP_X)).abs() + 0.6 * off(qy, top, bottom),
                Grow::Down if qy > pr.y + pr.h * 0.75 => (me.y - (pr.y + pr.h + GAP_V)).abs() + 0.6 * off(qx, left, right),
                _ => continue,
            };
            if best.map(|b| score < b.0).unwrap_or(true) {
                best = Some((score, p, dir));
            }
        }
    }
    best.filter(|b| b.0 <= JOIN_REACH).map(|b| (b.1, b.2))
}

/// Whether this shape is a root topic (one that can choose a structure).
pub fn is_root(d: &Diagram, id: u64) -> bool {
    let t = Tree::new(d);
    d.shape(id).map(|s| matches!(s.kind, ShapeKind::CentralTopic | ShapeKind::Topic)).unwrap_or(false) && !t.parent.contains_key(&id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DiagramKind, Project};

    fn shop() -> Diagram {
        Project::sample().diagrams.into_iter().find(|d| d.kind == DiagramKind::MindMap).unwrap()
    }
    fn id(d: &Diagram, name: &str) -> u64 {
        d.shapes.iter().find(|s| s.name == name).unwrap().id
    }

    #[test]
    fn balanced_sides_and_structures() {
        let mut d = shop();
        layout(&mut d);
        let c = d.shape(id(&d, "Online Shop")).unwrap().center().0;
        let right = ["Customers", "Products", "Payment", "Delivery"].iter().filter(|n| d.shape(id(&d, n)).unwrap().center().0 > c).count();
        assert_eq!(right, 2);
        // org chart: every sub-topic below its parent
        let root = id(&d, "Online Shop");
        d.shape_mut(root).unwrap().orient = ORG_CHART;
        layout(&mut d);
        let (r, k) = (d.shape(root).unwrap().clone(), d.shape(id(&d, "Customers")).unwrap().clone());
        assert_eq!(k.y, r.y + r.h + GAP_V);
        // logic chart: every sub-topic to the right
        d.shape_mut(root).unwrap().orient = LOGIC_CHART;
        layout(&mut d);
        let r = d.shape(root).unwrap().clone();
        assert!(d.shapes.iter().filter(|s| s.kind == ShapeKind::Topic).all(|s| s.x > r.x + r.w));
    }

    /// Add a sub-topic below `parent`, last in its order.
    fn add_kid(d: &mut Diagram, parent: u64, name: &str) {
        let nid = d.shapes.iter().map(|s| s.id).chain(d.connectors.iter().map(|c| c.id)).max().unwrap() + 1;
        let (px, py) = d.shape(parent).map(|p| (p.x, p.y)).unwrap();
        let mut s = d.shapes.iter().find(|s| s.kind == ShapeKind::Topic).unwrap().clone();
        s.id = nid;
        s.name = name.into();
        s.x = px + if px > 300.0 { 200.0 } else { -200.0 };
        s.y = py + 500.0;
        d.shapes.push(s);
        let mut c = d.connectors.iter().find(|c| c.kind == ConnKind::Branch).unwrap().clone();
        c.id = nid + 1;
        c.from = parent;
        c.to = nid;
        d.connectors.push(c);
    }

    #[test]
    fn sides_even_out_only_past_the_threshold() {
        // Customers 3 + Products 3 on the right, Payment 3 + Delivery 1 on the left: 6 against 4
        let mut d = shop();
        layout(&mut d);
        let root = d.shape(id(&d, "Online Shop")).unwrap().clone();
        let right_of = |d: &Diagram, n: &str| d.shape(id(d, n)).unwrap().center().0 > root.center().0;
        assert!(right_of(&d, "Customers") && right_of(&d, "Products"));
        // 7 against 4: past the threshold, but moving Products (3) over only turns it round
        let cus = id(&d, "Customers");
        add_kid(&mut d, cus, "Wish list");
        layout(&mut d);
        assert!(right_of(&d, "Customers") && right_of(&d, "Products"));
        // 8 against 4: sub-topics two levels down count too, so Products goes to the bottom left
        let wish = id(&d, "Wish list");
        add_kid(&mut d, wish, "Share");
        layout(&mut d);
        assert!(right_of(&d, "Customers") && !right_of(&d, "Products"));
        let (p, del) = (d.shape(id(&d, "Products")).unwrap().center().1, d.shape(id(&d, "Delivery")).unwrap().center().1);
        assert!(p > del, "Products joins the left side at its bottom");
        // left heavier by more than 2 (7 against 4): the lowest on the left comes back
        let pay = id(&d, "Payment");
        add_kid(&mut d, pay, "Wallet");
        add_kid(&mut d, pay, "Coupon");
        layout(&mut d);
        assert!(right_of(&d, "Products"));
    }

    #[test]
    fn fold_keeps_sub_topics_until_unfolded() {
        use crate::model::{mind_fold, mind_foldable, mind_unfold};
        let mut d = shop();
        layout(&mut d);
        let (shapes, conns) = (d.shapes.len(), d.connectors.len());
        let (root, products) = (id(&d, "Online Shop"), id(&d, "Products"));
        // never the central topic
        assert!(!mind_foldable(&d, root));
        assert!(mind_fold(&mut d, products));
        assert_eq!(d.shapes.len(), shapes - 2);
        assert!(!d.shapes.iter().any(|s| s.name == "Stock"));
        let knobs = fold_knobs(&d);
        assert!(knobs.iter().any(|k| k.0 == products && k.2 == Some(2)));
        layout(&mut d);
        // a pasted copy took Stock's id meanwhile: it comes back under a new one
        let stock_id = d.shape(products).unwrap().fold.as_ref().unwrap().shapes.iter().find(|s| s.name == "Stock").unwrap().id;
        let mut copy = d.shape(products).unwrap().clone();
        copy.id = stock_id;
        copy.fold = None;
        d.shapes.push(copy);
        let mut next = 10_000;
        assert!(mind_unfold(&mut d, products, || {
            next += 1;
            next
        }));
        assert_eq!((d.shapes.len(), d.connectors.len()), (shapes + 1, conns));
        let stock = id(&d, "Stock");
        assert_ne!(stock, stock_id);
        assert_eq!(crate::model::mind_parent(&d, stock), Some(products));
    }

    #[test]
    fn branches_take_the_style_of_the_topic_they_leave() {
        let mut d = shop();
        let (root, products, catalog) = (id(&d, "Online Shop"), id(&d, "Products"), id(&d, "Catalog"));
        let style = |d: &Diagram, to: u64| d.connectors.iter().find(|c| c.kind == ConnKind::Branch && c.to == to).map(|c| (c.style, c.from_anchor)).unwrap();
        layout(&mut d);
        assert_eq!(style(&d, products), (STYLE_FOLLOW_DIAGRAM, None));
        // the central topic's style reaches every level that sets none
        d.shape_mut(root).unwrap().branch_style = Some(STYLE_RECTILINEAR);
        layout(&mut d);
        assert_eq!(style(&d, products), (STYLE_RECTILINEAR, Some((1.0, 0.5))));
        assert_eq!(style(&d, catalog).0, STYLE_RECTILINEAR);
        // the branch-only styles keep the fixed ends their paths start from
        d.shape_mut(root).unwrap().branch_style = Some(STYLE_ARC);
        layout(&mut d);
        assert_eq!(style(&d, products), (STYLE_ARC, Some((1.0, 0.5))));
        let (a, b) = (d.shape(root).unwrap().clone(), d.shape(products).unwrap().clone());
        let c = d.connectors.iter().find(|c| c.to == products).unwrap().clone();
        let path = crate::render::conn_path(d.kind, &c, &a, &b, STYLE_FOLLOW_DIAGRAM);
        assert_eq!(path.first().copied(), Some((a.x + a.w, a.center().1)));
        assert_eq!(path.last().copied(), Some((b.x, b.center().1)));
        // an underlined topic's branches meet the line under its name
        d.shape_mut(root).unwrap().topic_shape = Some(crate::model::TOPIC_UNDERLINE);
        layout(&mut d);
        assert_eq!(style(&d, products), (STYLE_ARC, Some((1.0, 1.0))));
        d.shape_mut(root).unwrap().topic_shape = None;
        d.shape_mut(root).unwrap().branch_style = Some(STYLE_RECTILINEAR);
        layout(&mut d);
        // a topic's own style is for its branches only
        d.shape_mut(products).unwrap().branch_style = Some(STYLE_OBLIQUE);
        layout(&mut d);
        assert_eq!(style(&d, products).0, STYLE_RECTILINEAR);
        assert_eq!(style(&d, catalog).0, STYLE_OBLIQUE);
        // a branch's own style doesn't last
        let i = d.connectors.iter().position(|c| c.to == catalog).unwrap();
        d.connectors[i].style = STYLE_CURVE;
        layout(&mut d);
        assert_eq!(style(&d, catalog).0, STYLE_OBLIQUE);
    }

    #[test]
    fn arrow_keys_walk_the_tree() {
        let mut d = shop();
        layout(&mut d);
        let n = |d: &Diagram, s: &str| id(d, s);
        let (root, customers, products, catalog, stock, payment, card) = (n(&d, "Online Shop"), n(&d, "Customers"), n(&d, "Products"), n(&d, "Catalog"), n(&d, "Stock"), n(&d, "Payment"), n(&d, "Card"));
        // the root: each side's sub-topic nearest level with it
        let right = neighbour(&d, root, (1, 0)).unwrap();
        assert!(right == customers || right == products);
        assert_eq!(neighbour(&d, root, (-1, 0)).map(|k| k == payment || k == n(&d, "Delivery")), Some(true));
        // into the sub-topics, back to the parent, along the siblings
        assert!(matches!(neighbour(&d, products, (1, 0)), Some(k) if k == catalog || k == stock));
        assert_eq!(neighbour(&d, catalog, (-1, 0)), Some(products));
        assert_eq!(neighbour(&d, catalog, (0, 1)), Some(stock));
        assert_eq!(neighbour(&d, products, (0, -1)), Some(customers));
        // a leaf has nothing further out; the left side runs the other way
        assert_eq!(neighbour(&d, stock, (1, 0)), None);
        assert_eq!(neighbour(&d, payment, (1, 0)), Some(root));
        assert_eq!(neighbour(&d, payment, (-1, 0)).map(|k| k == card || k == n(&d, "Bank transfer")), Some(true));
        // org chart: down into the sub-topics, up to the parent, sideways along the row
        d.shape_mut(root).unwrap().orient = ORG_CHART;
        layout(&mut d);
        assert_eq!(neighbour(&d, catalog, (0, -1)), Some(products));
        assert!(matches!(neighbour(&d, products, (0, 1)), Some(k) if k == catalog || k == stock));
        assert!(neighbour(&d, catalog, (1, 0)).is_some() || neighbour(&d, catalog, (-1, 0)).is_some());
    }

    #[test]
    fn drop_joins_the_column_it_lands_in() {
        let mut d = shop();
        layout(&mut d);
        let products = d.shape(id(&d, "Products")).unwrap().clone();
        let stock = d.shape(id(&d, "Stock")).unwrap().clone();
        let del = id(&d, "Delivery");
        // under Products' sub-topics: joins Products
        let s = d.shape_mut(del).unwrap();
        s.x = stock.x;
        s.y = stock.y + 60.0;
        assert_eq!(drop_parent(&d, del).map(|t| t.0), Some(products.id));
        // far away: floats free
        let s = d.shape_mut(del).unwrap();
        s.y += 2000.0;
        assert_eq!(drop_parent(&d, del), None);
        // never into its own sub-tree
        let pid = products.id;
        let s = d.shape_mut(pid).unwrap();
        s.x = stock.x + 200.0;
        assert_ne!(drop_parent(&d, pid).map(|t| t.0), Some(stock.id));
    }
}
