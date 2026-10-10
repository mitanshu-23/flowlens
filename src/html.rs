//! A self-contained interactive explorer.
//!
//! A whole-crate graph drawn at once is a wall — 133 nodes of it. What people
//! actually want is the question an IDE answers: *for this function, what does
//! it take, what does it do, who calls it, what does it call?* So this emits
//! one HTML file holding the index as JSON plus a small app that shows exactly
//! that, one function at a time, with everything clickable.
//!
//! No build step, no network: open the file and it works.

use std::fmt::Write as _;

use crate::json::{json_list, json_str};
use crate::model::*;
use crate::provenance::Provenance;
use crate::workspace::Workspace;

pub fn emit(ws: &Workspace) -> String {
    let data = to_json(ws);
    let title = ws.info.name.clone().unwrap_or_else(|| "rust crate".to_string());

    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title} — code map</title>\n<style>\n{CSS}\n</style>\n</head>\n\
         <body>\n{BODY}\n<script>\nconst DATA = {data};\n{JS}\n</script>\n</body>\n</html>\n"
    )
}

// ------------------------------------------------------------------- data

fn to_json(ws: &Workspace) -> String {
    let mut out = String::new();
    out.push('{');

    write!(out, "\"crate\":{},", json_str(ws.info.name.as_deref().unwrap_or("crate"))).ok();

    // ---- files
    out.push_str("\"files\":[");
    for (i, f) in ws.files.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let module = if f.module_path.is_empty() { "crate".to_string() } else { format!("crate::{}", f.module_path.join("::")) };
        write!(out, "{{\"path\":{},\"name\":{},\"module\":{}}}", json_str(&f.path), json_str(f.path.rsplit('/').next().unwrap_or(&f.path)), json_str(&module)).ok();
    }
    out.push_str("],");

    // ---- symbols, flattened to one id space for the browser
    let mut flat: Vec<(FileId, SymbolId)> = Vec::new();
    let mut index: Vec<Vec<usize>> = ws.files.iter().map(|f| vec![0; f.symbols.len()]).collect();
    for (fid, f) in ws.files.iter().enumerate() {
        for s in &f.symbols {
            if !s.kind.is_callable() {
                continue;
            }
            index[fid][s.id] = flat.len();
            flat.push((fid, s.id));
        }
    }

    out.push_str("\"fns\":[");
    for (i, (fid, sid)) in flat.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let s = &ws.files[*fid].symbols[*sid];
        out.push('{');
        write!(
            out,
            "\"file\":{fid},\"name\":{},\"qualified\":{},\"owner\":{},\"line\":{},\"vis\":{},\"async\":{},\"ret\":{},\"doc\":{},\"sig\":{}",
            json_str(&s.name),
            json_str(&s.qualified),
            s.owner.as_deref().map(json_str).unwrap_or_else(|| "null".into()),
            s.loc.line,
            json_str(match s.visibility {
                Visibility::Public => "pub",
                Visibility::Crate => "pub(crate)",
                Visibility::Private => "private",
            }),
            s.is_async,
            s.ret.as_deref().map(json_str).unwrap_or_else(|| "null".into()),
            s.doc.as_deref().map(json_str).unwrap_or_else(|| "null".into()),
            json_str(&s.signature),
        )
        .ok();
        out.push_str(",\"params\":[");
        for (j, p) in s.params.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            write!(out, "{{\"name\":{},\"ty\":{}}}", json_str(&p.name), json_str(&p.ty)).ok();
        }
        out.push_str("]}");
    }
    out.push_str("],");

    // ---- calls between the user's own functions
    out.push_str("\"calls\":[");
    let mut first = true;
    for (fid, f) in ws.files.iter().enumerate() {
        for r in &f.references {
            if !r.is_call() {
                continue;
            }
            let Some(from_sym) = r.from else { continue };
            if !f.symbols[from_sym].kind.is_callable() {
                continue;
            }
            let Some(target) = r.global_target(fid) else {
                continue;
            };
            if !ws.symbol(target).kind.is_callable() {
                continue;
            }
            if !first {
                out.push(',');
            }
            first = false;
            write!(out, "{{\"from\":{},\"to\":{},\"line\":{},\"guess\":{},\"macro\":{},\"args\":{},\"bind\":{},\"ctx\":{}}}", index[fid][from_sym], index[target.file][target.sym], r.loc.line, r.by_name, r.via_macro, json_list(&r.args), r.binding.as_deref().map(json_str).unwrap_or_else(|| "null".into()), json_ctx(&r.context)).ok();
        }
    }
    out.push_str("],");

    // ---- library calls, kept separate and summarised per function
    out.push_str("\"lib\":[");
    let mut first = true;
    for (fid, f) in ws.files.iter().enumerate() {
        for r in &f.references {
            if !r.is_call() || r.provenance() != Provenance::ThirdParty {
                continue;
            }
            let Some(from_sym) = r.from else { continue };
            if !f.symbols[from_sym].kind.is_callable() {
                continue;
            }
            let Resolution::Unresolved { origin, .. } = &r.to else {
                continue;
            };
            if !first {
                out.push(',');
            }
            first = false;
            write!(out, "{{\"from\":{},\"name\":{},\"origin\":{},\"line\":{},\"args\":{},\"bind\":{},\"ctx\":{}}}", index[fid][from_sym], json_str(&r.path), json_str(origin.as_deref().unwrap_or("std")), r.loc.line, json_list(&r.args), r.binding.as_deref().map(json_str).unwrap_or_else(|| "null".into()), json_ctx(&r.context)).ok();
        }
    }
    out.push_str("]}");
    out
}

/// The control-flow stack as JSON: kind, label, and the offset that identifies
/// the construct so consecutive calls can be grouped under the same loop.
fn json_ctx(frames: &[ControlFrame]) -> String {
    let mut out = String::from("[");
    for (i, f) in frames.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(out, "{{\"k\":{},\"l\":{},\"o\":{},\"line\":{}}}", json_str(f.kind.tag()), json_str(&f.label), f.offset, f.line).ok();
    }
    out.push(']');
    out
}

// ------------------------------------------------------------------ page

const BODY: &str = r##"<header>
  <h1 id="crateName">crate</h1>
  <input id="search" type="search" placeholder="Search functions…  (press /)" autocomplete="off" spellcheck="false">
  <span id="counts"></span>
</header>

<main>
  <nav id="sidebar">
    <div class="nav-head">
      <label><input type="checkbox" id="onlyDocumented"> documented only</label>
      <label><input type="checkbox" id="onlyPublic"> public only</label>
    </div>
    <div id="list"></div>
  </nav>

  <section id="detail">
    <div class="empty">
      <h2>Pick a function</h2>
      <p>Search on the left, or start from an entry point below.</p>
      <div id="roots"></div>
    </div>
  </section>
</main>
"##;

const CSS: &str = r##"
:root {
  --bg: #ffffff; --panel: #f7f9fb; --line: #dde3ea; --ink: #1b2733;
  --muted: #62748a; --accent: #2563eb; --accent-soft: #e8f0fe;
  --pub: #16a34a; --pub-bg: #e6f7ec; --crate: #b45309; --crate-bg: #fef3c7;
  --priv: #64748b; --priv-bg: #eef2f6; --lib: #94a3b8;
  --mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #12171e; --panel: #171d26; --line: #2a3441; --ink: #e6edf5;
    --muted: #8ba0b8; --accent: #63a4ff; --accent-soft: #1c2b45;
    --pub: #4ade80; --pub-bg: #14301f; --crate: #fbbf24; --crate-bg: #362a10;
    --priv: #94a3b8; --priv-bg: #1e2632; --lib: #64748b;
  }
}
* { box-sizing: border-box; }
body {
  margin: 0; background: var(--bg); color: var(--ink);
  font: 14px/1.55 system-ui, -apple-system, "Segoe UI", sans-serif;
  height: 100vh; display: flex; flex-direction: column;
}
header {
  display: flex; align-items: center; gap: 16px; padding: 10px 18px;
  border-bottom: 1px solid var(--line); background: var(--panel); flex: none;
}
h1 { font-size: 15px; margin: 0; font-weight: 650; letter-spacing: -0.01em; }
#search {
  flex: 1; max-width: 460px; padding: 7px 11px; font: inherit;
  border: 1px solid var(--line); border-radius: 7px;
  background: var(--bg); color: var(--ink);
}
#search:focus { outline: 2px solid var(--accent); outline-offset: -1px; border-color: transparent; }
#counts { color: var(--muted); font-size: 12px; margin-left: auto; }

main { display: flex; flex: 1; min-height: 0; }
#sidebar {
  width: 320px; flex: none; border-right: 1px solid var(--line);
  background: var(--panel); display: flex; flex-direction: column; min-height: 0;
}
.nav-head {
  padding: 8px 14px; border-bottom: 1px solid var(--line);
  display: flex; gap: 14px; font-size: 12px; color: var(--muted);
}
.nav-head label { display: flex; align-items: center; gap: 5px; cursor: pointer; }
#list { overflow-y: auto; padding: 6px 0 24px; }

.file-group { margin-top: 4px; }
.file-name {
  padding: 6px 14px 4px; font-size: 11px; font-weight: 650;
  color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em;
  position: sticky; top: 0; background: var(--panel);
  display: flex; align-items: baseline; gap: 8px;
}
.hide-btn {
  border: 0; background: none; color: var(--muted); cursor: pointer;
  font: inherit; font-size: 10px; padding: 0 4px; margin-left: auto;
  opacity: 0; border-radius: 4px; text-transform: none; letter-spacing: 0;
}
.file-name:hover .hide-btn, .item-row:hover .hide-btn { opacity: 1; }
.hide-btn:hover { color: var(--accent); background: var(--accent-soft); }
.item-row { display: flex; align-items: center; }
.item-row .item { flex: 1; min-width: 0; }
.row-hide { font-size: 14px; line-height: 1; padding: 0 8px; flex: none; }
.restore {
  display: flex; align-items: center; gap: 10px; margin: 6px 10px 4px;
  padding: 5px 10px; font-size: 11.5px; color: var(--muted);
  background: var(--accent-soft); border-radius: 6px;
}
button.link {
  border: 0; background: none; color: var(--accent); cursor: pointer;
  font: inherit; text-decoration: underline; padding: 0; margin-left: auto;
}
.item {
  display: flex; align-items: baseline; gap: 7px; width: 100%;
  padding: 4px 14px 4px 22px; border: 0; background: none; cursor: pointer;
  font: inherit; color: var(--ink); text-align: left;
}
.item:hover { background: var(--accent-soft); }
.item[aria-current="true"] { background: var(--accent-soft); box-shadow: inset 3px 0 0 var(--accent); }
.item .nm { font-family: var(--mono); font-size: 12.5px; }
.item .ln { color: var(--muted); font-size: 11px; margin-left: auto; }
.dot { width: 6px; height: 6px; border-radius: 50%; flex: none; }
.dot.pub { background: var(--pub); }
.dot.crate { background: var(--crate); }
.dot.private { background: var(--priv); }

#detail { flex: 1; overflow-y: auto; padding: 26px 34px 60px; min-width: 0; }
.empty { color: var(--muted); max-width: 560px; }
.empty h2 { color: var(--ink); font-size: 17px; margin: 0 0 6px; }

.sig-block {
  font-family: var(--mono); font-size: 13px; background: var(--panel);
  border: 1px solid var(--line); border-radius: 9px; padding: 14px 16px;
  margin: 0 0 16px; white-space: pre-wrap; line-height: 1.7; overflow-x: auto;
}
.kw { color: var(--accent); }
.pname { color: var(--ink); font-weight: 600; }
.ptype { color: var(--muted); }
.fname { font-weight: 700; }

.title-row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin-bottom: 4px; }
.title-row h2 { font-family: var(--mono); font-size: 19px; margin: 0; font-weight: 650; }
.badge {
  font-size: 11px; padding: 2px 8px; border-radius: 999px; font-weight: 600;
}
.badge.pub { background: var(--pub-bg); color: var(--pub); }
.badge.crate { background: var(--crate-bg); color: var(--crate); }
.badge.private { background: var(--priv-bg); color: var(--priv); }
.where { color: var(--muted); font-size: 12.5px; margin-bottom: 18px; font-family: var(--mono); }

.doc {
  border-left: 3px solid var(--accent); padding: 2px 0 2px 14px;
  margin: 0 0 20px; white-space: pre-wrap; max-width: 74ch;
}
.doc.none { border-color: var(--line); color: var(--muted); font-style: italic; }
.doc code, .sig-block code { font-family: var(--mono); }

h3 {
  font-size: 11px; text-transform: uppercase; letter-spacing: 0.05em;
  color: var(--muted); margin: 24px 0 8px; font-weight: 650;
}
.rows { display: flex; flex-direction: column; gap: 1px; }
.row {
  display: flex; align-items: baseline; gap: 10px; padding: 6px 10px;
  border: 0; background: none; font: inherit; color: var(--ink);
  cursor: pointer; text-align: left; border-radius: 6px; width: 100%;
}
.row:hover { background: var(--accent-soft); }
.row .nm { font-family: var(--mono); font-size: 12.5px; }
.row .meta { color: var(--muted); font-size: 11.5px; margin-left: auto; white-space: nowrap; }
.row .tag {
  font-size: 10px; padding: 1px 6px; border-radius: 4px;
  background: var(--crate-bg); color: var(--crate); font-weight: 600;
}
.none-msg { color: var(--muted); font-size: 13px; padding: 4px 10px; }
.lib-item { padding: 5px 10px; display: flex; gap: 10px; align-items: baseline; }
.lib-item .nm { font-family: var(--mono); font-size: 12.5px; color: var(--lib); }
.lib-item .meta { color: var(--muted); font-size: 11.5px; margin-left: auto; }

.flow-head { display: flex; align-items: center; gap: 16px; }
.toggle {
  font-size: 12px; color: var(--muted); display: flex; align-items: center;
  gap: 5px; cursor: pointer; margin-top: 16px;
}
.toggle.plain {
  border: 1px solid var(--line); background: var(--bg); color: var(--muted);
  border-radius: 6px; padding: 3px 9px; font-family: inherit;
}
.toggle.plain:hover { border-color: var(--accent); color: var(--accent); }
.flow { border-left: 2px solid var(--line); }
.step {
  display: flex; gap: 12px; padding: 5px 0 5px 14px; margin-left: -2px;
  border-left: 2px solid transparent; align-items: baseline;
}
.step:hover { border-left-color: var(--accent); background: var(--accent-soft); }
.step.lib { opacity: 0.62; }
.step.long { cursor: zoom-in; }
.step.long.open, .flow.all-open .step.long { cursor: zoom-out; }
/* A clipped step shows an affordance so it is clear there is more to see. */
.step.long > .step-body::after {
  content: " \2026 more"; color: var(--accent); font-size: 11px;
}
.step.long.open > .step-body::after,
.flow.all-open .step.long > .step-body::after { content: none; }

/* A loop or branch the calls inside it sit under. */
.frame { margin: 6px 0 6px 14px; }
.frame-head {
  display: flex; align-items: baseline; gap: 8px; padding: 3px 0;
  font-family: var(--mono); font-size: 12px;
}
.frame-ln { color: var(--muted); font-size: 11px; min-width: 42px; text-align: right; flex: none; }
.frame-kw { font-weight: 700; }
.frame-label { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.frame-note {
  margin-left: auto; font-size: 10px; padding: 1px 7px; border-radius: 999px;
  font-family: system-ui, sans-serif; white-space: nowrap;
}
.frame-body { border-left: 2px solid var(--line); margin-left: 42px; }
.frame.repeats > .frame-head .frame-kw { color: var(--crate); }
.frame.repeats > .frame-head .frame-note { background: var(--crate-bg); color: var(--crate); }
.frame.repeats > .frame-body { border-left-color: var(--crate); }
.frame.if > .frame-head .frame-kw, .frame.else > .frame-head .frame-kw,
.frame.arm > .frame-head .frame-kw { color: var(--accent); }
.frame.if > .frame-head .frame-note, .frame.else > .frame-head .frame-note,
.frame.arm > .frame-head .frame-note { background: var(--accent-soft); color: var(--accent); }
.frame.if > .frame-body, .frame.else > .frame-body, .frame.arm > .frame-body {
  border-left-color: var(--accent); border-left-style: dashed;
}
.frame.closure > .frame-head .frame-kw { color: var(--muted); }
.frame.closure > .frame-head .frame-note { background: var(--priv-bg); color: var(--priv); }
.step-ln {
  font-family: var(--mono); font-size: 11px; color: var(--muted);
  min-width: 42px; text-align: right; flex: none;
}
.step-body {
  font-family: var(--mono); font-size: 12.5px; line-height: 1.6;
  flex: 1; min-width: 0;
  /* Collapsed: newlines fold to spaces and the overflow is clipped. */
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
ol.flow li.open > .step-body,
ol.flow.all-open li > .step-body {
  white-space: pre-wrap; overflow: visible; text-overflow: clip;
  word-break: break-word;
}
.bind { color: var(--pub); font-weight: 600; }
.args { color: var(--muted); }
.ret { color: var(--accent); }
.lib-origin { color: var(--lib); }
.lib-nm { color: var(--lib); }
button.linked {
  border: 0; background: none; padding: 0; font: inherit; cursor: pointer;
  color: var(--ink); font-weight: 600; text-decoration: underline;
  text-decoration-color: var(--line); text-underline-offset: 3px;
}
button.linked:hover { color: var(--accent); text-decoration-color: var(--accent); }
ol.flow .tag { margin-left: 8px; }

#map { width: 100%; margin-top: 10px; overflow-x: auto; }
#map svg { display: block; }
#map .node { cursor: pointer; }
#map .node rect { fill: var(--panel); stroke: var(--line); }
#map .node.self rect { fill: var(--accent-soft); stroke: var(--accent); stroke-width: 2; }
#map .node text { font-family: var(--mono); font-size: 11px; fill: var(--ink); }
#map .node:hover rect { stroke: var(--accent); }
#map path { fill: none; stroke: var(--line); stroke-width: 1.5; }
"##;

const JS: &str = r##"
const $ = (id) => document.getElementById(id);

// Callers and callees, built once from the flat call list.
const callers = DATA.fns.map(() => []);
const callees = DATA.fns.map(() => []);
for (const c of DATA.calls) {
  callees[c.from].push(c);
  callers[c.to].push(c);
}
const libByFn = DATA.fns.map(() => []);
for (const l of DATA.lib) libByFn[l.from].push(l);

DATA.fns.forEach((f, i) => { f.id = i; });

let selected = null;
let showLib = false;

// Functions and files the user has hidden. Kept per crate in localStorage so a
// tidied-up view survives a reload.
const STORE = 'flowlens:hidden:' + DATA.crate;
let hidden = { fns: new Set(), files: new Set() };
try {
  const saved = JSON.parse(localStorage.getItem(STORE) || '{}');
  hidden.fns = new Set(saved.fns || []);
  hidden.files = new Set(saved.files || []);
} catch (_) { /* corrupt or unavailable storage is not worth failing over */ }

function saveHidden() {
  try {
    localStorage.setItem(STORE, JSON.stringify({
      fns: [...hidden.fns], files: [...hidden.files],
    }));
  } catch (_) { /* private mode; the session still works */ }
}

/** Hidden either in its own right, or because its whole file is hidden. */
function isHidden(id) {
  const f = DATA.fns[id];
  return hidden.fns.has(f.qualified + '@' + f.file) || hidden.files.has(f.file);
}

function hideKey(id) {
  const f = DATA.fns[id];
  return f.qualified + '@' + f.file;
}

function hiddenCount() {
  let n = 0;
  for (let i = 0; i < DATA.fns.length; i++) if (isHidden(i)) n++;
  return n;
}

// ------------------------------------------------------------- rendering

const esc = (s) =>
  String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

const fileOf = (f) => DATA.files[f.file];
const label = (f) => (f.owner ? f.owner + '::' + f.name : f.name);

/** Signature laid out one parameter per line, so types are readable. */
function signatureHTML(f) {
  let out = '';
  if (f.async) out += '<span class="kw">async </span>';
  out += '<span class="kw">fn </span><span class="fname">' + esc(f.name) + '</span>(';
  if (f.params.length === 0) {
    out += ')';
  } else if (f.params.length === 1 && f.params[0].name === 'self') {
    out += '<span class="ptype">' + esc(f.params[0].ty) + '</span>)';
  } else {
    out += '\n';
    for (const p of f.params) {
      // A receiver is written whole (`&mut self`), not as `name: type`.
      out += p.name === 'self'
        ? '    <span class="ptype">' + esc(p.ty) + '</span>,\n'
        : '    <span class="pname">' + esc(p.name) + '</span>: <span class="ptype">' + esc(p.ty) + '</span>,\n';
    }
    out += ')';
  }
  if (f.ret) out += ' -&gt; <span class="ptype">' + esc(f.ret) + '</span>';
  return out;
}

/** The flow, nested inside the loops and branches the calls actually sit in.
 *
 *  A flat list misleads: a call inside a `for` runs many times, a call inside
 *  an `if` may not run at all. Consecutive steps sharing a construct are
 *  grouped under it, so the list reads as the shape of the body.
 */
function renderFlow(steps) {
  let h = '<div class="flow">';
  let open = [];                       // frames currently open, identified by offset

  for (const st of steps) {
    const ctx = st.ctx || [];
    let same = 0;
    while (same < open.length && same < ctx.length && open[same].o === ctx[same].o) same++;

    for (let i = open.length; i > same; i--) h += '</div></div>';
    for (let i = same; i < ctx.length; i++) h += frameOpen(ctx[i]);
    open = ctx;
    h += stepHTML(st);
  }
  for (let i = open.length; i > 0; i--) h += '</div></div>';
  return h + '</div>';
}

function frameOpen(fr) {
  const repeats = fr.k === 'for' || fr.k === 'while' || fr.k === 'loop';
  const note = repeats ? 'runs repeatedly'
             : fr.k === 'closure' ? 'deferred'
             : 'may not run';
  // The keyword is already in the label for `for`/`while`/`if`; do not repeat it.
  const text = fr.l.replace(/^(for|while|if) /, '');
  return '<div class="frame ' + fr.k + (repeats ? ' repeats' : '') + '">' +
    '<div class="frame-head"><span class="frame-ln">' + fr.line + '</span>' +
    '<span class="frame-kw">' + esc(fr.k) + '</span>' +
    '<span class="frame-label">' + esc(text) + '</span>' +
    '<span class="frame-note">' + note + '</span></div>' +
    '<div class="frame-body">';
}

function stepHTML(st) {
  const callee = st.lib ? null : DATA.fns[st.to];
  const nm = st.lib ? st.name : label(callee);
  const long = st.args.join(', ').length > 56;

  let h = '<div class="step ' + (st.lib ? 'lib ' : '') + (long ? 'long' : '') + '">';
  h += '<span class="step-ln">' + st.line + '</span>';
  h += '<span class="step-body"' + (long ? ' title="click to expand"' : '') + '>';
  if (st.bind) h += '<span class="bind">' + esc(st.bind) + '</span> = ';
  h += st.lib
    ? '<span class="nm lib-nm">' + esc(nm) + '</span>'
    : '<button class="nm linked" data-id="' + st.to + '">' + esc(nm) + '</button>';
  h += '<span class="args">' + argsHTML(st.args) + '</span>';
  if (callee && callee.ret) {
    h += '<span class="ret"> \u2192 ' + esc(callee.ret) + '</span>';
  } else if (st.lib) {
    h += '<span class="ret lib-origin"> \u2190 ' + esc(st.origin) + '</span>';
  }
  const tags = [];
  if (st.guess) tags.push('matched by name');
  if (st.macro) tags.push('inside a macro');
  if (tags.length) h += '<span class="tag">' + tags.join(' \u00b7 ') + '</span>';
  h += '</span></div>';
  return h;
}

/** Arguments as text. Newlines are written in but only *shown* when the step
 *  is expanded — collapsed, `white-space: nowrap` folds them back to spaces.
 *  So nothing is ever discarded, and one piece of markup serves both views. */
function argsHTML(args) {
  if (!args.length) return '()';
  const flat = args.join(', ');
  if (flat.length <= 56) return '(' + esc(flat) + ')';
  return '(\n' + args.map((a) => '    ' + esc(a)).join(',\n') + ',\n)';
}

function rowHTML(f, meta, tag) {
  return '<button class="row" data-id="' + f.id + '">' +
    '<span class="dot ' + f.vis.replace(/[()]/g, '').replace('pubcrate', 'crate') + '"></span>' +
    '<span class="nm">' + esc(label(f)) + '</span>' +
    (tag ? '<span class="tag">' + esc(tag) + '</span>' : '') +
    '<span class="meta">' + esc(meta) + '</span></button>';
}

function visClass(vis) {
  if (vis === 'pub') return 'pub';
  if (vis === 'private') return 'private';
  return 'crate';
}

function renderDetail(id) {
  selected = id;
  const f = DATA.fns[id];
  const file = fileOf(f);
  const ins = callers[id];
  const outs = callees[id];
  const libs = libByFn[id];

  let h = '';
  h += '<div class="title-row"><h2>' + esc(label(f)) + '</h2>' +
       '<span class="badge ' + visClass(f.vis) + '">' + esc(f.vis) + '</span></div>';
  h += '<div class="where">' + esc(file.module) + ' &middot; ' + esc(file.path) + ':' + f.line + '</div>';
  h += '<div class="sig-block">' + signatureHTML(f) + '</div>';
  h += f.doc
    ? '<div class="doc">' + esc(f.doc) + '</div>'
    : '<div class="doc none">No documentation.</div>';

  h += '<div id="map"></div>';

  // Group repeated call sites so a function called five times is one row.
  const group = (list, key) => {
    const m = new Map();
    for (const c of list) {
      const k = key(c);
      if (!m.has(k)) m.set(k, []);
      m.get(k).push(c);
    }
    return m;
  };

  h += '<h3>Called by (' + ins.length + ')</h3><div class="rows">';
  if (!ins.length) {
    h += '<div class="none-msg">Nothing in this crate calls it — an entry point, public API, or dead code.</div>';
  } else {
    for (const [from, cs] of group(ins.filter((c) => !isHidden(c.from)), (c) => c.from)) {
      const g = DATA.fns[from];
      const lines = cs.map((c) => c.line).sort((a, b) => a - b);
      h += rowHTML(g, fileOf(g).name + ':' + lines.join(', '), cs.some((c) => c.guess) ? 'by name' : '');
    }
  }
  h += '</div>';

  // Calls, in the order they run. Grouping by target would lose the one thing
  // you want when reading a function: what happens first.
  const flow = outs
    .filter((c) => !isHidden(c.to))
    .map((c) => ({
      line: c.line, args: c.args, bind: c.bind, to: c.to, ctx: c.ctx,
      guess: c.guess, macro: c.macro, lib: false,
    }));
  const libFlow = libs.map((l) => ({
    line: l.line, args: l.args, bind: l.bind, name: l.name, ctx: l.ctx,
    origin: l.origin, lib: true,
  }));

  h += '<div class="flow-head"><h3>Calls in order (' + flow.length + ')</h3>' +
       (libs.length
         ? '<label class="toggle"><input type="checkbox" id="withLib"' +
           (showLib ? ' checked' : '') + '> show ' + libs.length + ' library calls inline</label>'
         : '') +
       '<button class="toggle plain" id="expandAll">expand all</button>' +
       '</div>';

  const steps = (showLib ? flow.concat(libFlow) : flow).sort((a, b) => a.line - b.line);
  h += steps.length
    ? renderFlow(steps)
    : '<div class="none-msg">Calls nothing else in this crate.</div>';

  $('detail').innerHTML = h;
  drawMap(id);

  const expand = $('expandAll');
  if (expand) {
    expand.addEventListener('click', () => {
      const list = document.querySelector('ol.flow');
      if (!list) return;
      const open = list.classList.toggle('all-open');
      expand.textContent = open ? 'collapse all' : 'expand all';
    });
  }

  const lib = $('withLib');
  if (lib) {
    lib.addEventListener('change', () => {
      showLib = lib.checked;
      const keep = $('detail').scrollTop;
      renderDetail(id);
      $('detail').scrollTop = keep;
    });
  }

  for (const el of document.querySelectorAll('#list .item')) {
    el.setAttribute('aria-current', el.dataset.id === String(id) ? 'true' : 'false');
  }
  history.replaceState(null, '', '#' + encodeURIComponent(f.qualified + '@' + file.path));
}

/** Callers above, this function in the middle, callees below. */
function drawMap(id) {
  const up = [...new Set(callers[id].map((c) => c.from))].slice(0, 8);
  const down = [...new Set(callees[id].map((c) => c.to))].slice(0, 8);
  if (!up.length && !down.length) return;

  const W = 150, H = 30, GAPX = 14, GAPY = 62;
  const rowWidth = (n) => n * W + (n - 1) * GAPX;
  const width = Math.max(rowWidth(up.length || 1), rowWidth(down.length || 1), W) + 20;
  const rows = 1 + (up.length ? 1 : 0) + (down.length ? 1 : 0);
  const height = rows * H + (rows - 1) * (GAPY - H) + 20;

  const place = (ids, y) =>
    ids.map((n, i) => ({
      id: n,
      x: (width - rowWidth(ids.length)) / 2 + i * (W + GAPX),
      y,
    }));

  let y = 10;
  const upNodes = up.length ? place(up, y) : [];
  if (up.length) y += GAPY;
  const selfNode = { id, x: (width - W) / 2, y };
  if (down.length) y += GAPY;
  const downNodes = down.length ? place(down, y) : [];

  const curve = (a, b) =>
    'M' + (a.x + W / 2) + ',' + (a.y + H) +
    'C' + (a.x + W / 2) + ',' + (a.y + H + 24) + ' ' +
    (b.x + W / 2) + ',' + (b.y - 24) + ' ' + (b.x + W / 2) + ',' + b.y;

  let svg = '<svg width="' + width + '" height="' + height + '" viewBox="0 0 ' + width + ' ' + height + '">';
  for (const n of upNodes) svg += '<path d="' + curve(n, selfNode) + '"/>';
  for (const n of downNodes) svg += '<path d="' + curve(selfNode, n) + '"/>';

  const node = (n, isSelf) => {
    const f = DATA.fns[n.id];
    let t = label(f);
    if (t.length > 20) t = t.slice(0, 19) + '…';
    return '<g class="node' + (isSelf ? ' self' : '') + '" data-id="' + n.id + '">' +
      '<rect x="' + n.x + '" y="' + n.y + '" width="' + W + '" height="' + H + '" rx="7"/>' +
      '<text x="' + (n.x + W / 2) + '" y="' + (n.y + H / 2 + 4) + '" text-anchor="middle">' +
      esc(t) + '</text><title>' + esc(f.qualified) + '</title></g>';
  };
  for (const n of upNodes) svg += node(n, false);
  svg += node(selfNode, true);
  for (const n of downNodes) svg += node(n, false);
  svg += '</svg>';

  $('map').innerHTML = svg;
}

// --------------------------------------------------------------- sidebar

function renderList() {
  const q = $('search').value.trim().toLowerCase();
  const docOnly = $('onlyDocumented').checked;
  const pubOnly = $('onlyPublic').checked;

  const matches = DATA.fns.filter((f) => {
    if (isHidden(f.id)) return false;
    if (docOnly && !f.doc) return false;
    if (pubOnly && f.vis === 'private') return false;
    if (!q) return true;
    return f.qualified.toLowerCase().includes(q) ||
           (f.doc || '').toLowerCase().includes(q) ||
           fileOf(f).name.toLowerCase().includes(q);
  });

  const gone = hiddenCount();
  $('counts').textContent =
    matches.length + ' of ' + DATA.fns.length + ' fn \u00b7 ' + DATA.files.length + ' files' +
    (gone ? ' \u00b7 ' + gone + ' hidden' : '');

  const byFile = new Map();
  for (const f of matches) {
    if (!byFile.has(f.file)) byFile.set(f.file, []);
    byFile.get(f.file).push(f);
  }

  let h = '';
  if (gone) {
    h += '<div class="restore"><span>' + gone + ' hidden</span>' +
         '<button class="link" id="showAll">show all</button></div>';
  }
  for (const [fid, fns] of [...byFile].sort((a, b) => a[0] - b[0])) {
    h += '<div class="file-group"><div class="file-name">' +
         '<span>' + esc(DATA.files[fid].name) + '</span>' +
         '<button class="hide-btn" data-hide-file="' + fid +
         '" title="hide this whole file">hide file</button></div>';
    for (const f of fns.sort((a, b) => a.line - b.line)) {
      h += '<div class="item-row">' +
        '<button class="item" data-id="' + f.id + '" aria-current="' + (f.id === selected) + '">' +
        '<span class="dot ' + visClass(f.vis) + '"></span>' +
        '<span class="nm">' + esc(label(f)) + '</span>' +
        '<span class="ln">' + f.line + '</span></button>' +
        '<button class="hide-btn row-hide" data-hide-fn="' + f.id +
        '" title="hide this function">\u00d7</button></div>';
    }
    h += '</div>';
  }
  $('list').innerHTML = h || '<div class="none-msg">Nothing matches.</div>';
}

/** Functions nothing in the crate calls — the natural places to start. */
function renderRoots() {
  const roots = DATA.fns
    .filter((f, i) => callers[i].length === 0 && callees[i].length > 0)
    .sort((a, b) => callees[b.id].length - callees[a.id].length)
    .slice(0, 12);
  const el = $('roots');
  if (!el) return;
  el.innerHTML = '<h3>Entry points</h3><div class="rows">' +
    roots.map((f) => rowHTML(f, fileOf(f).name + ' · calls ' + callees[f.id].length)).join('') +
    '</div>';
}

// ---------------------------------------------------------------- events

document.addEventListener('click', (e) => {
  const hideFn = e.target.closest('[data-hide-fn]');
  if (hideFn) {
    hidden.fns.add(hideKey(Number(hideFn.dataset.hideFn)));
    saveHidden();
    renderList();
    if (selected !== null) renderDetail(selected);
    return;
  }
  const hideFile = e.target.closest('[data-hide-file]');
  if (hideFile) {
    hidden.files.add(Number(hideFile.dataset.hideFile));
    saveHidden();
    renderList();
    if (selected !== null) renderDetail(selected);
    return;
  }
  if (e.target.id === 'showAll') {
    hidden = { fns: new Set(), files: new Set() };
    saveHidden();
    renderList();
    if (selected !== null) renderDetail(selected);
    return;
  }

  // A click on the callee name navigates; anywhere else on a clipped step
  // expands it, so long arguments are always reachable.
  const hit = e.target.closest('[data-id]');
  if (hit) {
    renderDetail(Number(hit.dataset.id));
    return;
  }
  const step = e.target.closest('.step');
  if (step) step.classList.toggle('open');
});
$('search').addEventListener('input', renderList);
$('onlyDocumented').addEventListener('change', renderList);
$('onlyPublic').addEventListener('change', renderList);
document.addEventListener('keydown', (e) => {
  if (e.key === '/' && document.activeElement !== $('search')) {
    e.preventDefault();
    $('search').focus();
    $('search').select();
  }
});

$('crateName').textContent = DATA.crate;
document.title = DATA.crate + ' — code map';
renderList();
renderRoots();

// Deep link back to a function, so a shared URL lands in the right place.
if (location.hash) {
  const want = decodeURIComponent(location.hash.slice(1));
  const i = DATA.fns.findIndex((f) => f.qualified + '@' + fileOf(f).path === want);
  if (i >= 0) renderDetail(i);
}
"##;
