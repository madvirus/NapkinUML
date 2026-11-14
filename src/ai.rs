//! AI Assistant: draw or change diagrams by talking to an AI agent installed on this
//! computer (Claude Code, Codex CLI, Gemini CLI, Ollama, or any command the person sets).
//!
//! The agent is run as a separate program with the request on its standard input. The
//! request carries the open diagram as JSON, the shape and line kinds each diagram type
//! allows, and the conversation so far; the agent answers with one JSON object:
//! `{"reply": "...", "diagram": {"kind", "name", "new", "shapes", "connectors"}}`.
//! The diagram in the answer is the whole diagram after the change; shapes and lines that
//! keep their ids keep every property the answer doesn't mention.

use crate::model::{ConnKind, Connector, Diagram, DiagramKind, Project, Shape, ShapeKind};
use crate::palette::{palette, Tool};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Longest an agent may take before it is stopped.
pub const TIMEOUT: Duration = Duration::from_secs(600);

/// The agents Craft knows how to run: (id, name shown, program to look for).
pub const AGENTS: [(&str, &str, &str); 4] = [
    ("claude", "Claude Code", "claude"),
    ("codex", "Codex CLI", "codex"),
    ("gemini", "Gemini CLI", "gemini"),
    ("ollama", "Ollama", "ollama"),
];

/// An agent found on this computer.
#[derive(Clone, Debug)]
pub struct Found {
    pub id: &'static str,
    pub label: &'static str,
    pub path: PathBuf,
}

/// Look for each known agent on the PATH (and in the usual install folders).
pub fn detect() -> Vec<Found> {
    let mut out = vec![];
    for (id, label, prog) in AGENTS {
        if let Some(path) = find_program(prog) {
            out.push(Found { id, label, path });
        }
    }
    out
}

fn find_program(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    // installers that don't always reach the PATH of programs started from Explorer
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from) {
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".claude").join("local"));
        dirs.push(home.join("AppData").join("Roaming").join("npm"));
        dirs.push(home.join("AppData").join("Local").join("Programs").join("Ollama"));
        dirs.push(home.join(".npm-global").join("bin"));
    }
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    let exts: Vec<String> = if cfg!(windows) { vec![".exe".into(), ".cmd".into(), ".bat".into()] } else { vec![String::new()] };
    for d in dirs {
        for e in &exts {
            let p = d.join(format!("{name}{e}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

// ---------- the request ----------

/// Drop empty and default-looking fields so the diagram sent to the agent stays short.
fn compact(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut o = Map::new();
            for (k, x) in m {
                let keep = match x {
                    Value::Null => false,
                    Value::Bool(b) => *b,
                    Value::String(s) => !s.is_empty(),
                    Value::Array(a) => !a.is_empty() && !(a.len() == 2 && a.iter().all(|n| n.as_f64() == Some(0.0))),
                    _ => true,
                };
                // presentation details the agent never needs
                let internal = matches!(k.as_str(), "style" | "nav_from" | "nav_to" | "label_offset" | "from_anchor" | "to_anchor" | "orient" | "font" | "font_size" | "documentation" | "seq_no");
                if keep && !internal {
                    o.insert(k.clone(), compact(x));
                }
            }
            Value::Object(o)
        }
        Value::Array(a) => Value::Array(a.iter().map(compact).collect()),
        Value::Number(n) => {
            // whole numbers read better than 120.0
            match n.as_f64() {
                Some(f) if f.fract() == 0.0 && f.abs() < 1e12 => json!(f as i64),
                _ => v.clone(),
            }
        }
        _ => v.clone(),
    }
}

fn kinds_table() -> String {
    let mut s = String::new();
    for k in DiagramKind::ALL {
        let mut shapes = vec![];
        let mut lines = vec![];
        for e in palette(k) {
            match e.tool {
                Tool::Shape(sk) => shapes.push(format!("{sk:?}")),
                Tool::Conn(ck) => lines.push(format!("{ck:?}")),
                Tool::Select => {}
            }
        }
        s.push_str(&format!("- {:?}: shapes {}; connectors {}\n", k, shapes.join(", "), lines.join(", ")));
    }
    s
}

/// Everything the agent needs for one request.
pub fn build_prompt(project: Option<&Project>, active: Option<&Diagram>, history: &[(bool, String)], request: &str) -> String {
    let mut p = String::new();
    p.push_str(
        "You are the drawing assistant inside NapkinUML, a UML diagram editor. The person asks, in plain words and in any language, to draw a diagram, change the open one, or asks a question about it.\n\
Do not use any tools, do not read or write files and do not run commands: just answer.\n\
Answer with exactly ONE JSON object and nothing else (no explanation around it):\n\
{\"reply\": \"<one or two short sentences to the person, in the person's language>\", \"diagram\": {\"kind\": \"<diagram kind>\", \"name\": \"<diagram name>\", \"new\": false, \"shapes\": [...], \"connectors\": [...]}}\n\
Leave \"diagram\" out when nothing on the diagram should change (for example, a question).\n\
\"diagram\" is the WHOLE diagram after the change: repeat every shape and connector that stays (with the same id), change what was asked, add new ones with new ids, and leave out the ones to delete. Set \"new\": true (and a kind and name) to make a separate new diagram instead of changing the open one; do that when the person asks for a new diagram or when no diagram is open, or when the kind of diagram asked for differs from the open one.\n\n\
Shape fields: id (number, unique), kind, x, y (top-left corner, pixels, y grows downwards), w, h, name, stereotype, attributes (list of text lines, e.g. \"-name : String\"; ERD columns like \"PK id : int\" or \"name : varchar(50)\"; state lines like \"entry / start()\"), operations (list of lines, e.g. \"+total() : double\"), fill (optional [r, g, b, 255]).\n\
Connector fields: id (number, unique), kind, from (shape id), to (shape id), name, from_label and to_label (multiplicities or roles such as \"1\", \"0..*\"), arrow_end true to show the navigability arrow on an association; for sequence-diagram messages also y (the message's vertical position). A note's Anchor connector goes from the Note to the shape it describes.\n\n\
Layout rules: shapes must not overlap (except containment); leave 60-120 px between shapes; make shapes large enough for their text (about 7 px per character, 18 px per line plus 30 px for the name). To put shapes inside a Package, Component, Node, Device, ExecutionEnvironment, System, Instance, swimlane or fragment, place them fully inside its rectangle. Sequence diagrams: Lifeline or ActorLifeline shapes side by side at y = 40, about 140 px apart, tall enough to hold all messages (h 300 or more); messages (Message, AsyncMessage, ReturnMessage, CreateMessage) go between lifelines with y starting at 110 and growing by about 40 in time order. Activity and state diagrams flow top to bottom from the Initial node. Network diagrams: device shapes (Router, Switch, Firewall, Server, Database, Pc, Person...) are small pictures, about 60 x 50, with the name drawn below them, so leave about 40 px under each; put the devices of one segment (LAN, DMZ, VLAN) inside a NetworkZone; use Link for cables and Wireless for wireless connections. Note and Text shapes may hold several lines separated by \\n.\n\n\
Diagram kinds, with the shape and connector kinds each allows (use exactly these names):\n",
    );
    p.push_str(&kinds_table());
    if let Some(pr) = project {
        let others: Vec<String> = pr.diagrams.iter().filter(|d| Some(d.id) != active.map(|a| a.id)).map(|d| format!("{} ({:?})", d.name, d.kind)).collect();
        if !others.is_empty() {
            p.push_str(&format!("\nOther diagrams in the project: {}\n", others.join(", ")));
        }
    }
    match active {
        Some(d) => {
            let v = json!({ "kind": d.kind, "name": d.name, "shapes": d.shapes, "connectors": d.connectors });
            p.push_str(&format!("\nThe open diagram:\n{}\n", serde_json::to_string(&compact(&v)).unwrap_or_default()));
        }
        None => p.push_str("\nNo diagram is open.\n"),
    }
    let recent: Vec<&(bool, String)> = history.iter().rev().take(10).collect();
    if !recent.is_empty() {
        p.push_str("\nThe conversation so far:\n");
        for (user, text) in recent.into_iter().rev() {
            p.push_str(&format!("{}: {}\n", if *user { "Person" } else { "Assistant" }, text));
        }
    }
    p.push_str(&format!("\nThe person's request:\n{request}\n"));
    p
}

// ---------- running the agent ----------

/// A request being answered in the background.
pub struct Job {
    pub rx: Receiver<Result<String, String>>,
    pub cancel: Arc<AtomicBool>,
    pub started: Instant,
}

/// Which agent to run and how.
#[derive(Clone, Debug)]
pub enum Agent {
    Known(Found),
    Custom(String),
}

/// Split a command line into words (double quotes group words with spaces).
fn split_command(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for ch in s.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The first model `ollama list` shows.
fn first_ollama_model(path: &PathBuf) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.arg("list").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    no_window(&mut cmd);
    let out = cmd.output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().skip(1).find_map(|l| l.split_whitespace().next().map(|s| s.to_string()))
}

#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console window flashes up
}
#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}

/// Start answering `prompt` with `agent` in the background.
pub fn start(agent: Agent, model: String, prompt: String) -> Job {
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    std::thread::spawn(move || {
        let _ = tx.send(run(agent, &model, &prompt, &stop));
    });
    Job { rx, cancel, started: Instant::now() }
}

/// Run the agent to the end and return what it answered.
pub fn run(agent: Agent, model: &str, prompt: &str, stop: &AtomicBool) -> Result<String, String> {
    let work = std::env::temp_dir().join("napkinuml-ai");
    let _ = std::fs::create_dir_all(&work);
    let last_message = work.join(format!("last-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&last_message);
    let mut cmd = match &agent {
        Agent::Known(f) => {
            let mut c = Command::new(&f.path);
            match f.id {
                "claude" => {
                    c.args(["-p", "--output-format", "text"]);
                }
                "codex" => {
                    c.args(["exec", "--skip-git-repo-check", "--output-last-message"]).arg(&last_message).arg("-");
                }
                "gemini" => {}
                "ollama" => {
                    let m = if model.trim().is_empty() { first_ollama_model(&f.path).ok_or("Ollama has no models. Install one first, for example: ollama pull llama3.1")? } else { model.trim().to_string() };
                    c.args(["run", &m]);
                }
                _ => {}
            }
            c
        }
        Agent::Custom(line) => {
            let words = split_command(line);
            let Some((prog, args)) = words.split_first() else { return Err("No command is set for the custom AI agent.".into()) };
            let mut c = Command::new(prog);
            c.args(args);
            c
        }
    };
    // a Claude Code started from inside another Claude Code session would refuse to run
    cmd.env_remove("CLAUDECODE").env_remove("CLAUDE_CODE_ENTRYPOINT");
    cmd.current_dir(&work).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    no_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("Could not start the AI agent: {e}"))?;
    let mut stdin = child.stdin.take().unwrap();
    let input = prompt.to_string();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let out_t = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = so.read_to_end(&mut b);
        b
    });
    let err_t = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = se.read_to_end(&mut b);
        b
    });
    let started = Instant::now();
    let status = loop {
        if let Ok(Some(st)) = child.try_wait() {
            break st;
        }
        if stop.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Stopped.".into());
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err("The AI agent took too long and was stopped.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = writer.join();
    let out = String::from_utf8_lossy(&out_t.join().unwrap_or_default()).to_string();
    let err = String::from_utf8_lossy(&err_t.join().unwrap_or_default()).to_string();
    let answer = std::fs::read_to_string(&last_message).ok().filter(|s| !s.trim().is_empty()).unwrap_or(out);
    let _ = std::fs::remove_file(&last_message);
    if answer.trim().is_empty() {
        let why = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no answer").trim().to_string();
        return Err(format!("The AI agent gave no answer (exit {}): {why}", status.code().unwrap_or(-1)));
    }
    Ok(answer)
}

// ---------- the answer ----------

/// What the agent answered.
#[derive(Debug, Default)]
pub struct Reply {
    pub message: String,
    pub diagram: Option<Value>,
}

/// The JSON object inside the agent's text (it may be wrapped in a ``` block or prose).
fn find_json(text: &str) -> Option<Value> {
    let mut candidates: Vec<&str> = vec![];
    for part in text.split("```").skip(1).step_by(2) {
        candidates.push(part.trim_start_matches("json").trim_start_matches("JSON"));
    }
    if let (Some(a), Some(b)) = (text.find('{'), text.rfind('}')) {
        if a < b {
            candidates.push(&text[a..=b]);
        }
    }
    candidates.push(text);
    for c in candidates {
        if let Ok(v) = serde_json::from_str::<Value>(c.trim()) {
            if v.is_object() {
                return Some(v);
            }
        }
    }
    None
}

pub fn parse_reply(text: &str) -> Reply {
    match find_json(text) {
        Some(v) => {
            let message = v.get("reply").and_then(|r| r.as_str()).unwrap_or("").to_string();
            let diagram = v.get("diagram").filter(|d| d.is_object()).cloned();
            // an answer that is itself a diagram
            let diagram = diagram.or_else(|| if v.get("shapes").is_some() { Some(v.clone()) } else { None });
            Reply { message, diagram }
        }
        None => Reply { message: text.trim().to_string(), diagram: None },
    }
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

/// Overlay the answer's fields on an existing item (so unmentioned properties stay).
fn overlay(base: Option<Value>, item: &Map<String, Value>) -> Map<String, Value> {
    let mut m = match base {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    for (k, v) in item {
        m.insert(k.clone(), v.clone());
    }
    m
}

fn kind_from(v: Option<&Value>) -> Option<DiagramKind> {
    let s = v?.as_str()?.replace([' ', '-', '_'], "").to_lowercase();
    let s = s.trim_end_matches("diagram");
    DiagramKind::ALL.into_iter().find(|k| format!("{k:?}").to_lowercase() == s || (s == "entityrelationship" && *k == DiagramKind::Erd) || (s == "state" && *k == DiagramKind::StateMachine))
}

/// What applying an answer did.
pub struct Applied {
    /// The diagram that was changed or made.
    pub diagram: u64,
    pub created: bool,
    pub shapes: usize,
    pub connectors: usize,
}

/// Put the answer's diagram into the project: change the open diagram, or make a new one.
pub fn apply(project: &mut Project, active: Option<u64>, d: &Value) -> Result<Applied, String> {
    let kind = kind_from(d.get("kind"));
    let name = d.get("name").and_then(|n| n.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let open = active.and_then(|id| project.diagram(id)).map(|x| (x.id, x.kind));
    let want_new = d.get("new").and_then(|b| b.as_bool()).unwrap_or(false);
    let (target, created) = match open {
        Some((id, k)) if !want_new && kind.map(|kk| kk == k).unwrap_or(true) => (id, false),
        _ => {
            let k = kind.or(open.map(|o| o.1)).unwrap_or(DiagramKind::Class);
            (project.add_diagram(k, name.clone()), true)
        }
    };
    let old =project.diagram(target).cloned().unwrap();
    let old_shapes: HashMap<u64, Value> = old.shapes.iter().map(|s| (s.id, serde_json::to_value(s).unwrap_or_default())).collect();
    let old_conns: HashMap<u64, Value> = old.connectors.iter().map(|c| (c.id, serde_json::to_value(c).unwrap_or_default())).collect();
    let empty = vec![];
    let shapes_in = d.get("shapes").and_then(|s| s.as_array()).unwrap_or(&empty);
    let conns_in = d.get("connectors").or_else(|| d.get("lines")).and_then(|s| s.as_array()).unwrap_or(&empty);

    // ids: kept when they are this diagram's, otherwise given fresh project ids
    let mut ids: HashMap<String, u64> = HashMap::new();
    let mut taken: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut map_id = |raw: &Value, existing: &dyn Fn(u64) -> bool, project: &mut Project| -> u64 {
        let key = match raw {
            Value::Null => format!("auto{}", ids.len()),
            v => v.to_string().trim_matches('"').to_string(),
        };
        if let Some(&id) = ids.get(&key) {
            return id;
        }
        let id = match key.parse::<u64>() {
            Ok(n) if existing(n) && !taken.contains(&n) => n,
            _ => project.fresh_id(),
        };
        taken.insert(id);
        ids.insert(key, id);
        id
    };
    let mut shapes: Vec<Shape> = vec![];
    for item in shapes_in.iter().filter_map(|v| v.as_object()) {
        let id = map_id(item.get("id").unwrap_or(&Value::Null), &|n| old_shapes.contains_key(&n), project);
        let mut m = overlay(old_shapes.get(&id).cloned(), item);
        m.insert("id".into(), json!(id));
        let Some(kind) = m.get("kind").and_then(|k| serde_json::from_value::<ShapeKind>(k.clone()).ok()) else { continue };
        let (dw, dh) = kind.default_size();
        for (k, dv) in [("x", 40.0), ("y", 40.0), ("w", dw as f64), ("h", dh as f64)] {
            let v = m.get(k).and_then(num).unwrap_or(dv);
            m.insert(k.into(), json!(v));
        }
        if !m.get("name").map(|n| n.is_string()).unwrap_or(false) {
            m.insert("name".into(), json!(""));
        }
        for list in ["attributes", "operations"] {
            if let Some(Value::String(s)) = m.get(list).cloned() {
                m.insert(list.into(), json!(s.split('\n').collect::<Vec<_>>()));
            }
        }
        if m.get("stereotype").map(|s| !s.is_string()).unwrap_or(false) {
            m.remove("stereotype");
        }
        if let Ok(s) = serde_json::from_value::<Shape>(Value::Object(m)) {
            shapes.push(s);
        }
    }
    // lines first get their own ids (a note may be anchored to a line)
    let conn_ids: Vec<u64> = conns_in.iter().map(|item| map_id(item.get("id").unwrap_or(&Value::Null), &|n| old_conns.contains_key(&n), project)).collect();
    let mut connectors: Vec<Connector> = vec![];
    for (item, id) in conns_in.iter().zip(conn_ids) {
        let Some(item) = item.as_object() else { continue };
        let mut m = overlay(old_conns.get(&id).cloned(), item);
        m.insert("id".into(), json!(id));
        if m.get("kind").and_then(|k| serde_json::from_value::<ConnKind>(k.clone()).ok()).is_none() {
            continue;
        }
        let mut ok = true;
        for end in ["from", "to"] {
            let raw = item.get(end).or_else(|| m.get(end)).cloned().unwrap_or(Value::Null);
            let key = raw.to_string().trim_matches('"').to_string();
            match ids.get(&key) {
                Some(&n) => {
                    m.insert(end.into(), json!(n));
                }
                None => ok = false,
            }
        }
        if !ok {
            continue;
        }
        if let Some(y) = m.get("y").and_then(num) {
            m.insert("y".into(), json!(y));
        }
        if let Ok(c) = serde_json::from_value::<Connector>(Value::Object(m)) {
            connectors.push(c);
        }
    }
    // a line must join things that exist on this diagram
    let alive: std::collections::HashSet<u64> = shapes.iter().map(|s| s.id).chain(connectors.iter().map(|c| c.id)).collect();
    connectors.retain(|c| alive.contains(&c.from) && alive.contains(&c.to));
    if shapes.is_empty() && connectors.is_empty() && !shapes_in.is_empty() {
        if created {
            project.diagrams.retain(|x| x.id != target);
        }
        return Err("The AI answer had shapes, but none of them could be read.".into());
    }
    let (ns, nc) = (shapes.len(), connectors.len());
    let dm = project.diagram_mut(target).unwrap();
    dm.shapes = shapes;
    dm.connectors = connectors;
    // (the open diagram keeps its name; a new one was named when it was made)
    project.repair_ids();
    Ok(Applied { diagram: target, created, shapes: ns, connectors: nc })
}

/// Answer one request without a window (`--ai`): load the project (or start a new one),
/// run the agent set in the AI Assistant, apply its answer to the first diagram and save.
pub fn headless(settings: &crate::settings::Settings, request: &str, input: Option<&std::path::Path>, out: &std::path::Path) -> Result<String, String> {
    let mut project = match input {
        Some(p) => {
            let text = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
            let mut pr: Project = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            pr.repair_ids();
            pr
        }
        None => Project::new("Untitled"),
    };
    let found = detect();
    let agent = if settings.ai_agent == "custom" {
        Agent::Custom(settings.ai_command.clone())
    } else {
        Agent::Known(found.iter().find(|f| f.id == settings.ai_agent).or_else(|| found.first()).cloned().ok_or("No AI agent was found on this computer.")?)
    };
    let active = project.diagrams.first().map(|d| d.id);
    let prompt = build_prompt(Some(&project), active.and_then(|id| project.diagram(id)), &[], request);
    let text = run(agent, &settings.ai_model, &prompt, &AtomicBool::new(false))?;
    let reply = parse_reply(&text);
    let mut msg = reply.message.clone();
    if let Some(d) = reply.diagram {
        let done = apply(&mut project, active, &d)?;
        msg.push_str(&format!("\n[{} shapes, {} connectors, {}]", done.shapes, done.connectors, if done.created { "new diagram" } else { "changed" }));
    } else if msg.is_empty() {
        msg = text;
    }
    std::fs::write(out, serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(msg)
}
