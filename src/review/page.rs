//! The review page: one self-contained HTML file, no server, no network.
//!
//! Written for someone who will not read the code. The page is a map of the
//! project — functions as boxes, calls as arrows, flowing from the features
//! on the left to what they use on the right — coloured by what changed.
//! With many changes, the map is what keeps them readable: where they are and
//! what leads to them is visible before a single word is read.
//!
//! Clicking a box opens that function as a small flowchart of its steps,
//! changed ones marked. The code is one click further, for the reader who
//! wants it.

use std::fmt::Write as _;

use super::compare::Row;
use super::flow::Step;
use super::{FnReview, Review};
use crate::json::{json_list, json_str};

/// The mark: a cog-toothed lens with a flow running through it — a step and
/// an arrow — and a handle. Drawn in `currentColor`, so it takes the page's rust.
const LOGO: &str = r#"<svg viewBox="0 0 32 32" aria-hidden="true"><circle cx="14" cy="14" r="12" fill="none" stroke="currentColor" stroke-width="4" stroke-dasharray="4.2 5.22"/><circle cx="14" cy="14" r="10" fill="none" stroke="currentColor" stroke-width="2.6"/><circle cx="8.6" cy="14" r="2.4" fill="currentColor"/><path d="M10.5 14H16.4" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M15.6 10.4 20.4 14 15.6 17.6Z" fill="currentColor" stroke="currentColor" stroke-width="1" stroke-linejoin="round"/><path d="M22.6 22.6 29 29" stroke="currentColor" stroke-width="4" stroke-linecap="round"/></svg>"#;

/// The mark as the browser tab's icon, in rust, as a data URL.
fn favicon() -> String {
    let svg = LOGO.replace("currentColor", "#b7410e").replace(" aria-hidden=\"true\"", " xmlns=\"http://www.w3.org/2000/svg\"");
    format!("data:image/svg+xml,{}", svg.replace('"', "'").replace('#', "%23").replace('<', "%3C").replace('>', "%3E"))
}

pub fn emit(review: &Review) -> String {
    let data = to_json(review);
    let title = format!("{} — what changed", review.crate_name);
    let body = BODY.replace("LOGO_SVG", LOGO);
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<link rel=\"icon\" href=\"{}\">\n<style>\n{CSS}\n</style>\n</head>\n\
         <body>\n{body}\n<script>\nconst DATA = {data};\n{JS}\n</script>\n</body>\n</html>\n",
        title.replace('<', "&lt;"),
        favicon()
    )
}

// ------------------------------------------------------------------- data

fn to_json(r: &Review) -> String {
    let mut out = String::from("{");
    write!(out, "\"crate\":{},\"against\":{},\"history\":{},", json_str(&r.crate_name), json_str(&r.against), r.has_history).ok();

    out.push_str("\"fns\":{");
    for (i, f) in r.fns.values().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(out, "{}:{}", json_str(&f.key), fn_json(f)).ok();
    }
    out.push_str("},");

    out.push_str("\"features\":[");
    for (i, f) in r.features.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(out, "{{\"k\":{},\"s\":{},\"ci\":{}}}", json_str(&f.key), json_str(f.status.tag()), json_list(&f.changed_inside)).ok();
    }
    write!(out, "],\"unreached\":{},\"tests\":{},\"glossary\":{{", json_list(&r.unreached), json_list(&r.tests)).ok();
    for (i, e) in r.glossary.values().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(out, "{}:{}", json_str(&e.key), entry_json(e)).ok();
    }
    out.push_str("}}");
    out
}

fn entry_json(e: &super::glossary::Entry) -> String {
    let opt = |v: &Option<String>| v.as_deref().map(json_str).unwrap_or_else(|| "null".into());
    let parts: Vec<String> = e.parts.iter().map(|p| format!("[{},{},{}]", json_str(&p.name), json_str(&p.ty), opt(&p.doc))).collect();
    format!("{{\"n\":{},\"k\":{},\"doc\":{},\"f\":{},\"ln\":{},\"parts\":[{}],\"ty\":{},\"val\":{},\"src\":{},\"srcb\":{},\"ub\":{},\"s\":{}}}", json_str(&e.name), json_str(e.kind), opt(&e.doc), json_str(&e.file), e.line, parts.join(","), opt(&e.ty), opt(&e.value), json_str(&e.source), opt(&e.source_before), json_list(&e.used_by), json_str(e.status.tag()))
}

fn fn_json(f: &FnReview) -> String {
    let mut out = String::new();
    let sig = match &f.signature_change {
        Some((b, a)) => format!("[{},{}]", json_str(b), json_str(a)),
        None => "null".to_string(),
    };
    write!(
        out,
        "{{\"n\":{},\"o\":{},\"l\":{},\"f\":{},\"ln\":{},\"s\":{},\"sig\":{},\"t\":{},\"cb\":{},\"d\":{},\"cbe\":{},\"ce\":{},\"ft\":{},\"doc\":{},\"p\":[{}],\"r\":{},\"u\":{},\"v\":[{}],\"rows\":{},\"code\":[",
        json_str(&f.name),
        f.owner.as_deref().map(json_str).unwrap_or_else(|| "null".into()),
        json_str(&f.label),
        json_str(&f.file),
        f.line,
        json_str(f.status.tag()),
        sig,
        f.is_test,
        json_list(&f.changed_below),
        f.details_only(),
        json_list(&f.calls_before),
        json_list(&f.calls_now),
        f.is_feature,
        f.doc.as_deref().map(json_str).unwrap_or_else(|| "null".into()),
        f.params.iter().map(|(n, t)| format!("[{},{}]", json_str(n), json_str(t))).collect::<Vec<_>>().join(","),
        f.ret.as_deref().map(json_str).unwrap_or_else(|| "null".into()),
        json_list(&f.uses),
        f.vars
            .iter()
            .map(|v| format!("{{\"n\":{},\"m\":{},\"t\":{},\"i\":{},\"ln\":{},\"s\":[{}]}}", json_str(&v.name), v.mutable, v.ty.as_deref().map(json_str).unwrap_or_else(|| "null".into()), v.init.as_deref().map(json_str).unwrap_or_else(|| "null".into()), v.line, v.sets.iter().map(|(l, op, val)| format!("[{l},{},{}]", json_str(op), json_str(val))).collect::<Vec<_>>().join(",")))
            .collect::<Vec<_>>()
            .join(","),
        rows_json(&f.rows),
    )
    .ok();
    for (i, (mark, b, a, text)) in f.code.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let num = |n: &Option<usize>| n.map(|n| n.to_string()).unwrap_or_else(|| "0".into());
        write!(out, "[{},{},{},{}]", json_str(mark.tag()), num(b), num(a), json_str(text)).ok();
    }
    out.push_str("]}");
    out
}

fn rows_json(rows: &[Row]) -> String {
    let mut out = String::from("[");
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let step = |s: &Option<Step>| s.as_ref().map(step_json).unwrap_or_else(|| "null".into());
        write!(out, "{{\"m\":{},\"b\":{},\"a\":{},\"c\":{}}}", json_str(r.mark.tag()), step(&r.before), step(&r.after), rows_json(&r.children)).ok();
    }
    out.push(']');
    out
}

fn step_json(s: &Step) -> String {
    match s {
        Step::Call { target, args, fallible, crashes, handed, line, .. } => format!("{{\"t\":\"call\",\"k\":{},\"args\":{},\"f\":{fallible},\"x\":{crashes},\"h\":{},\"ln\":{line}}}", json_str(target), json_list(args), handed.as_deref().map(json_str).unwrap_or_else(|| "null".into())),
        Step::Set { var, op, value, line } => format!("{{\"t\":\"set\",\"v\":{},\"op\":{},\"val\":{},\"ln\":{line}}}", json_str(var), json_str(op), json_str(value)),
        Step::Exit { kind, detail, line } => format!("{{\"t\":\"exit\",\"e\":{},\"d\":{},\"ln\":{line}}}", json_str(kind.tag()), json_str(detail)),
        Step::Block { kind, header, on, on_call, line, .. } => format!("{{\"t\":\"block\",\"b\":{},\"h\":{},\"on\":{},\"oc\":{},\"ln\":{line}}}", json_str(kind.tag()), json_str(header), json_str(on), on_call.as_deref().map(json_str).unwrap_or_else(|| "null".into())),
    }
}

// ------------------------------------------------------------------- page

const BODY: &str = r##"<header class="bar">
  <div class="bar-top">
    <div class="title">
      <span class="brand" title="flowlens — see how your Rust code flows">LOGO_SVG<b>flowlens</b></span>
      <h1 id="crate"></h1>
      <span class="status" id="headline"></span>
      <span class="against" id="against"></span>
    </div>
    <div class="tools sys" aria-label="Page settings">
      <select class="toolbtn themesel" id="theme" title="Theme — kept in this browser" aria-label="Theme">
        <option value="">Theme: match my system (Light / Navy)</option>
        <option value="light">Theme: Light</option>
        <option value="rust">Theme: Rust</option>
        <option value="coal">Theme: Coal</option>
        <option value="navy">Theme: Navy</option>
        <option value="ayu">Theme: Ayu</option>
      </select>
      <select class="toolbtn themesel" id="font" title="Font for the page — kept in this browser" aria-label="Font">
        <option value="">Font: Grotesk</option>
        <option value="mono">Font: Monospace</option>
      </select>
      <button class="toolbtn" id="signsbtn" aria-expanded="false" title="What the signs, colours and numbers on the map and in the steps mean">Signs</button>
      <div class="rxpanel signpanel" id="signpanel" hidden role="dialog" aria-label="What the signs mean"></div>
    </div>
  </div>
  <div class="bar-tools tools">
    <div class="seg" role="group" aria-label="What to show">
      <button id="m-changes" class="on" title="Only what changed, and the paths from the features down to it">Changes</button>
      <button id="m-all" title="The whole program as the code is now, compared with nothing">Project flow</button>
    </div>
    <div class="seg icons" role="group" aria-label="Direction">
      <button id="o-h" class="on" title="Horizontal: features on the left, what they call to the right" aria-label="Horizontal">⇢</button>
      <button id="o-v" title="Vertical: features at the top, what they call below" aria-label="Vertical">⇣</button>
    </div>
    <div class="seg icons" role="group" aria-label="Where the details go">
      <button id="d-right" title="Details on the right" aria-label="Details on the right"><svg class="ic" viewBox="0 0 18 14" aria-hidden="true"><rect x="0.75" y="0.75" width="16.5" height="12.5" rx="2"/><rect class="f" x="11" y="0.75" width="6.25" height="12.5" rx="1"/></svg></button>
      <button id="d-left" title="Details on the left" aria-label="Details on the left"><svg class="ic" viewBox="0 0 18 14" aria-hidden="true"><rect x="0.75" y="0.75" width="16.5" height="12.5" rx="2"/><rect class="f" x="0.75" y="0.75" width="6.25" height="12.5" rx="1"/></svg></button>
      <button id="d-bottom" title="Details below the map" aria-label="Details below the map"><svg class="ic" viewBox="0 0 18 14" aria-hidden="true"><rect x="0.75" y="0.75" width="16.5" height="12.5" rx="2"/><rect class="f" x="0.75" y="8" width="16.5" height="5.25" rx="1"/></svg></button>
      <button id="d-window" title="Details in a separate window — handy on a second screen" aria-label="Details in a separate window"><svg class="ic" viewBox="0 0 18 14" aria-hidden="true"><rect x="0.75" y="3.75" width="12" height="9.5" rx="2"/><path d="M6 0.75h11.25v9"/></svg></button>
    </div>
    <button class="toolbtn" id="structbtn" title="Folders, files and what each defines — in the side space">▤ Structure</button>
    <input id="q" type="search" placeholder="Find a function  /" autocomplete="off" aria-label="Find a function">
    <button class="regexbtn" id="rxhelp" aria-expanded="false" title="Search patterns: ready-made ones, your own, and what the signs mean">?</button>
    <button class="regexbtn" id="regexbtn" aria-pressed="false" title="Regular expression: ^get matches names starting with get, _efile$ names ending with _efile. Or write /pattern/">.*</button>
    <span class="qcount" id="qcount" hidden aria-live="polite"></span>
    <div class="rxpanel" id="rxpanel" hidden role="dialog" aria-label="Search patterns"></div>
  </div>
</header>
<div class="split">
  <div class="maps" id="maps">
  <section class="mapwrap" id="mapwrap">
    <div class="sidetabs maintabs" id="maintabs"><span class="lead" title="The map of the whole project">Main space</span></div>
    <svg class="map" id="map" role="img" aria-label="Map of the project's functions and calls">
      <defs>
        <marker id="ar-same" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L8,4 L0,8 z"/></marker>
        <marker id="ar-add" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L8,4 L0,8 z"/></marker>
        <marker id="ar-del" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L8,4 L0,8 z"/></marker>
      </defs>
      <g class="vp"><g class="edges"></g><g class="nodes"></g></g>
    </svg>
    <div class="legend">
      <span><i class="sw changed"></i>Changed</span>
      <span><i class="sw added"></i>New</span>
      <span><i class="sw removed"></i>Removed</span>
      <span><i class="sw inside"></i>Leads to a change</span>
      <span><i class="sw same"></i>Unchanged</span>
    </div>
    <div class="zoom">
      <button class="z-in" aria-label="Zoom in">+</button>
      <button class="z-out" aria-label="Zoom out">−</button>
      <button class="z-fit" aria-label="Fit to screen">Fit</button>
    </div>
    <div class="maphint" id="maphint"></div>
    <div class="hiddenbox" id="hiddenbox" hidden>
      <button class="hiddenchip" id="hiddenchip" aria-expanded="false" title="Functions you hid from the map"></button>
      <div class="hiddenlist" id="hiddenlist" hidden></div>
    </div>
    <div class="toast" id="toast" hidden role="status"></div>
    <div class="focusbar" id="focusbar" hidden></div>
  </section>
  <div class="mapsgutter" id="mapsgutter" hidden role="separator" aria-label="Drag to share the space between the map and the side space; double-click to split evenly" tabindex="0"></div>
  <section class="mapwrap side" id="sidewrap" hidden aria-label="Side space: flows taken out of the main map">
    <div class="sidetabs" id="sidetabs"></div>
    <div class="sidetree" id="sidetree" hidden></div>
    <svg class="map" role="img" aria-label="A flow taken out of the main map">
      <g class="vp"><g class="edges"></g><g class="nodes"></g></g>
    </svg>
    <div class="zoom">
      <button class="z-in" aria-label="Zoom in">+</button>
      <button class="z-out" aria-label="Zoom out">−</button>
      <button class="z-fit" aria-label="Fit to screen">Fit</button>
    </div>
  </section>
  </div>
  <div class="gutter" id="gutter" role="separator" aria-label="Drag to resize the details" tabindex="0"></div>
  <aside class="panel" id="panel"></aside>
</div>"##;

const CSS: &str = r##"
/* Themes: the five of the Rust book (mdBook) — Light, Rust, Coal, Navy and
   Ayu. Without a choice the page does what the book does: Light, or Navy
   when the system is dark. A choice from the Theme menu sets data-theme on
   <html>. Every theme keeps the status colours' meaning (amber changed,
   green new, red removed); the accent is kept clear of the amber, and the
   logo stays rust. */
:root, :root[data-theme="light"] {
  --bg: #ffffff; --card: #fafafa; --ink: #1a1a1a; --muted: #747474; --line: #e6e6e6; --edge: #cccccc;
  --info: #eef4f8; --info-ink: #20609f; --same: #ffffff; --code: #f6f7f6; --focus: #20609f;
  --rust: #20609f; --rust-soft: #e3edf6; --copper: #b7410e; --grid: #ececec; --on-rust: #ffffff;
  --add: #e3f3e7; --add-ink: #1e6b34; --add-edge: #3c9a5a;
  --del: #fbe7e4; --del-ink: #a3291b; --del-edge: #c4503f;
  --chg: #fff1cc; --chg-ink: #865600; --chg-edge: #d39b1c;
  --brand: #b7410e; color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme]) {
  --bg: #161923; --card: #1b1f2c; --ink: #c8c9db; --muted: #8a8ca4; --line: #282d3f; --edge: #4a5068;
  --info: #262933; --info-ink: #9bbfe0; --same: #1b1f2c; --code: #1d2130; --focus: #3d8fc0;
  --rust: #3d8fc0; --rust-soft: #1f2a3c; --copper: #282d3f; --grid: #1e2230; --on-rust: #ffffff;
  --add: #15301d; --add-ink: #82d89d; --add-edge: #4fae6c;
  --del: #3a1c19; --del-ink: #f19a8d; --del-edge: #d0645a;
  --chg: #362a0e; --chg-ink: #f1c869; --chg-edge: #c9971f;
  --brand: #e2733d; color-scheme: dark;
  }
}
:root[data-theme="navy"] {
  --bg: #161923; --card: #1b1f2c; --ink: #c8c9db; --muted: #8a8ca4; --line: #282d3f; --edge: #4a5068;
  --info: #262933; --info-ink: #9bbfe0; --same: #1b1f2c; --code: #1d2130; --focus: #3d8fc0;
  --rust: #3d8fc0; --rust-soft: #1f2a3c; --copper: #282d3f; --grid: #1e2230; --on-rust: #ffffff;
  --add: #15301d; --add-ink: #82d89d; --add-edge: #4fae6c;
  --del: #3a1c19; --del-ink: #f19a8d; --del-edge: #d0645a;
  --chg: #362a0e; --chg-ink: #f1c869; --chg-edge: #c9971f;
  --brand: #e2733d; color-scheme: dark;
}
:root[data-theme="rust"] {
  --bg: #e1e1db; --card: #e9e9e4; --ink: #262625; --muted: #6e6b5e; --line: #cdc9bf; --edge: #b38f6b;
  --info: #d4d4ce; --info-ink: #2b6d91; --same: #eeeee9; --code: #d6d4cc; --focus: #a8541f;
  --rust: #a8541f; --rust-soft: #ead8c8; --copper: #3b2e2a; --grid: #d2d2cb; --on-rust: #ffffff;
  --add: #e3f3e7; --add-ink: #1e6b34; --add-edge: #3c9a5a;
  --del: #fbe7e4; --del-ink: #a3291b; --del-edge: #c4503f;
  --chg: #fff1cc; --chg-ink: #865600; --chg-edge: #d39b1c;
  --brand: #b7410e; color-scheme: light;
}
:root[data-theme="coal"] {
  --bg: #141617; --card: #1b1d1f; --ink: #b3c0cc; --muted: #7d8891; --line: #2b2f33; --edge: #43484d;
  --info: #242637; --info-ink: #8fb4e0; --same: #1b1d1f; --code: #1d1f21; --focus: #3f82c0;
  --rust: #3473ad; --rust-soft: #1d2a38; --copper: #292c2f; --grid: #1f2223; --on-rust: #ffffff;
  --add: #15301d; --add-ink: #82d89d; --add-edge: #4fae6c;
  --del: #3a1c19; --del-ink: #f19a8d; --del-edge: #d0645a;
  --chg: #362a0e; --chg-ink: #f1c869; --chg-edge: #c9971f;
  --brand: #e2733d; color-scheme: dark;
}
:root[data-theme="ayu"] {
  --bg: #0f1419; --card: #14191f; --ink: #c5c5c5; --muted: #8a9199; --line: #1f2630; --edge: #5c6773;
  --info: #1b2330; --info-ink: #73d0ff; --same: #14191f; --code: #191f26; --focus: #0096cf;
  --rust: #0096cf; --rust-soft: #102a38; --copper: #ffb454; --grid: #151b22; --on-rust: #ffffff;
  --add: #15301d; --add-ink: #82d89d; --add-edge: #4fae6c;
  --del: #3a1c19; --del-ink: #f19a8d; --del-edge: #d0645a;
  --chg: #362a0e; --chg-ink: #f1c869; --chg-edge: #c9971f;
  --brand: #e2733d; color-scheme: dark;
}
* { box-sizing: border-box; }
[hidden] { display: none !important; }
html, body { height: 100%; }
body { margin: 0; background: var(--bg); color: var(--ink); display: flex; flex-direction: column;
  font: 14px/1.45 var(--font); }
/* Fonts from the Font menu — Grotesk by default, or Monospace. Installed
   fonts only: the page works offline. */
:root { --font: "Nimbus Sans", Helvetica, "Helvetica Neue", Arial, "Liberation Sans", sans-serif; }
:root[data-font="mono"] { --font: ui-monospace, "DejaVu Sans Mono", "Ubuntu Mono", Menlo, Consolas, monospace; }
code, .mono { font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 0.88em; }
code { background: var(--code); padding: 0 4px; border-radius: 4px; overflow-wrap: anywhere; }
button { font: inherit; color: inherit; }

/* Two rows: the page itself — whose project, and its settings — on top;
   the tools for working with the map below. */
.bar { position: relative; border-bottom: 1px solid var(--line); background: var(--card); }
.bar-top { display: flex; gap: 8px 20px; align-items: center; justify-content: space-between; flex-wrap: wrap; padding: 7px 16px; }
.bar-tools { padding: 6px 16px 7px; border-top: 1px solid var(--line); background: var(--bg); }
.tools.sys { gap: 6px; }
.bar-tools #q { margin-left: auto; }
.tools.sys .toolbtn { height: 28px; font-size: 12.5px; }
/* A strip of hot metal along the bottom of the bar. */
.bar::after { content: ""; position: absolute; left: 0; right: 0; bottom: -1px; height: 2px;
  background: linear-gradient(90deg, var(--rust), var(--copper) 45%, transparent); pointer-events: none; }
.brand { display: inline-flex; align-items: center; gap: 7px; color: var(--brand); text-decoration: none; padding-right: 12px; border-right: 1px solid var(--line); }
.brand svg { width: 24px; height: 24px; flex: none; }
.brand b { font-size: 12.5px; font-weight: 800; letter-spacing: .02em; color: var(--ink); }
@media (max-width: 560px) { .brand b { display: none; } .brand { padding-right: 10px; } }
.title { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; min-width: 0; }
h1 { margin: 0; font-size: 15.5px; font-weight: 700; white-space: nowrap; font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
.status { display: inline-flex; align-items: center; height: 22px; padding: 0 9px; border-radius: 999px; font-size: 12px; font-weight: 600;
  border: 1px solid var(--line); color: var(--muted); white-space: nowrap; }
.status.changed { background: var(--chg); border-color: var(--chg-edge); color: var(--chg-ink); }
.against { color: var(--muted); font-size: 12px; white-space: nowrap; cursor: help; }
.against .mono { font-size: 11.5px; }
.tools { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
.seg { display: inline-flex; height: 30px; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; background: var(--bg); }
.seg button { border: none; background: none; height: 100%; padding: 0 12px; cursor: pointer; font-size: 13px; color: var(--muted); display: inline-flex; align-items: center; }
.seg button + button { border-left: 1px solid var(--line); }
.seg button:hover:not(:disabled):not(.on) { color: var(--ink); background: var(--card); }
.seg button.on { background: var(--rust); color: var(--on-rust); }
.seg button:disabled { opacity: .45; cursor: not-allowed; }
.seg.icons button { padding: 0 9px; font-size: 15px; }
.seg .ic { width: 18px; height: 14px; fill: none; stroke: currentColor; stroke-width: 1.5; }
.seg .ic .f { fill: currentColor; stroke: none; }
#q { height: 30px; border: 1px solid var(--line); background: var(--bg); color: var(--ink); border-radius: 8px; padding: 0 10px; font: inherit; font-size: 13px; width: 190px; }
#q:focus, .seg button:focus-visible, .toolbtn:focus-visible { outline: 2px solid var(--focus); outline-offset: 1px; }

.split { flex: 1; display: grid; min-height: 0; --panel-w: 440px; --panel-h: 40vh; }
.split.dock-right { grid-template-columns: 1fr 6px var(--panel-w); grid-template-areas: "map gutter panel"; }
.split.dock-left { grid-template-columns: var(--panel-w) 6px 1fr; grid-template-areas: "panel gutter map"; }
.split.dock-bottom { grid-template-rows: 1fr 6px var(--panel-h); grid-template-columns: 1fr; grid-template-areas: "map" "gutter" "panel"; }
.split.dock-window { grid-template-columns: 1fr; grid-template-areas: "map"; }
.split.dock-window .panel, .split.dock-window .gutter { display: none; }
/* Details closed: the map takes the room, and what is selected opens in a
   box over it instead. */
.split.closed { grid-template-columns: 1fr !important; grid-template-rows: 1fr !important; grid-template-areas: "map" !important; position: relative; }
.split.closed .gutter { display: none; }
.split.closed .panel { display: none; }
.split.closed.modal-on .panel { display: block; position: absolute; z-index: 20; top: 14px; right: 14px; bottom: 14px; width: min(var(--panel-w), calc(100% - 28px));
  border: 1px solid var(--line); border-radius: 12px; box-shadow: 0 12px 40px rgba(0, 0, 0, .35); }
.split.dock-left.closed.modal-on .panel { right: auto; left: 14px; }
.split.dock-bottom.closed.modal-on .panel { top: auto; left: 14px; right: 14px; width: auto; height: min(var(--panel-h), calc(100% - 28px)); }
.navclose { border: 1px solid var(--line); background: var(--bg); border-radius: 6px; padding: 2px 9px; font-size: 12.5px; cursor: pointer; flex: none; }
.navclose:hover { border-color: var(--del-edge); color: var(--del-ink); }
.maps { grid-area: map; display: flex; min-height: 0; min-width: 0; }
.maps > .mapwrap { flex: 1 1 0; min-width: 0; }
.mapwrap.side { background: var(--bg); }
.mapsgutter { flex: none; width: 6px; background: var(--line); cursor: col-resize; touch-action: none; }
.mapsgutter:hover, .mapsgutter.drag { background: var(--focus); }
.sidetabs { position: absolute; left: 0; right: 0; top: 0; z-index: 2; display: flex; gap: 4px; align-items: center; flex-wrap: wrap;
  padding: 6px 8px; background: var(--card); border-bottom: 1px solid var(--line); font-size: 12.5px; }
.sidetabs .lead { color: var(--muted); font-weight: 600; margin-right: 4px; }
.sidetab { display: inline-flex; align-items: center; border: 1px solid var(--line); border-radius: 999px; background: var(--bg); overflow: hidden; }
.sidetab button { border: none; background: none; cursor: pointer; padding: 2px 8px; font-size: 12.5px; }
.sidetab.on { border-color: var(--focus); background: var(--info); color: var(--info-ink); }
.sidetab .x { padding: 2px 7px 2px 2px; color: var(--muted); }
.sidetab .x:hover { color: var(--del-ink); }
.side .map { padding-top: 40px; }
/* The main map's name, small, in its top corner. */
.sidetabs.maintabs { right: auto; top: 8px; left: 10px; padding: 2px 9px; border: 1px solid var(--line); border-radius: 999px; background: var(--card); font-size: 11.5px; }
.maintabs .lead { margin: 0; }

.node.aside rect.box { stroke-dasharray: 6 3; stroke-width: 2; }
.node .asidechip text { fill: var(--info-ink); font-size: 10.5px; font-weight: 700; }
.node .asidechip rect { fill: var(--info); stroke: none; }
.hiddenbox { position: absolute; left: 12px; bottom: 56px; z-index: 5; }
.hiddenchip { border: 1px solid var(--line); background: var(--card); border-radius: 999px; padding: 3px 11px; font-size: 12.5px; cursor: pointer; color: var(--muted); }
.hiddenchip:hover, .hiddenchip[aria-expanded="true"] { border-color: var(--focus); color: var(--ink); }
.hiddenlist { position: absolute; left: 0; bottom: calc(100% + 6px); width: 300px; max-height: 50vh; overflow-y: auto; background: var(--card);
  border: 1px solid var(--line); border-radius: 10px; box-shadow: 0 8px 28px rgba(0,0,0,.18); padding: 8px 10px; font-size: 13px; }
.hiddenlist .hl-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px; }
.hiddenlist ul { list-style: none; margin: 0; padding: 0; }
.hiddenlist li { display: flex; justify-content: space-between; gap: 8px; align-items: baseline; padding: 4px 0; border-top: 1px solid var(--line); }
.hiddenlist li .mono { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.toast { position: absolute; left: 50%; bottom: 16px; transform: translateX(-50%); z-index: 6; display: flex; gap: 12px; align-items: center;
  background: var(--ink); color: var(--bg); border-radius: 8px; padding: 7px 12px; font-size: 13px; box-shadow: 0 6px 20px rgba(0,0,0,.25); }
.toast .link { color: var(--bg); border-color: rgba(255,255,255,.4); background: none; }
.hidebtn:hover rect { stroke: var(--del-edge); }
.hidebtn:hover text { fill: var(--del-ink); }
.tree li.ti.hiddenfn { opacity: .5; }
.tree li.ti.hiddenfn .lbl::after { content: ' · hidden'; font-size: 11px; color: var(--muted); }
.takeout { opacity: 0; transition: opacity .15s; cursor: pointer; }
.node:hover .takeout, .node:focus .takeout { opacity: 1; }
/* No square browser outline around a rounded box: a click already selects
   it, and keyboard focus lights the box's own rounded edge instead. */
svg.map g:focus { outline: none; }
.node:focus-visible rect.box, .proxy:focus-visible rect.box { stroke: var(--focus); stroke-width: 3; stroke-dasharray: none; }
.pg-btn:focus-visible rect { stroke: var(--focus); stroke-width: 2; }
.takeout rect { fill: var(--card); stroke: var(--line); }
.takeout text { fill: var(--muted); font-size: 12px; }
.takeout:hover rect { stroke: var(--focus); }
.takeout:hover text { fill: var(--focus); }
.panel { grid-area: panel; }
.gutter { grid-area: gutter; background: var(--line); cursor: col-resize; touch-action: none; }
.gutter:hover, .gutter.drag { background: var(--focus); }
.split.dock-bottom .gutter { cursor: row-resize; }
.split.dock-bottom .panel { border-left: none; }
.split.dock-bottom .maps { flex-direction: row; }
body.popup { display: block; height: 100%; }
body.popup .panel { border: none; height: 100%; overflow-y: auto; }
.popnote { margin: 0 0 10px; padding: 8px 10px; background: var(--info); color: var(--info-ink); border-radius: 8px; font-size: 13px; }
.mapwrap { position: relative; overflow: hidden; min-height: 0;
  background-color: var(--bg); background-image: radial-gradient(var(--grid) 1.1px, transparent 1.3px); background-size: 22px 22px; }
.map { width: 100%; height: 100%; display: block; cursor: grab; touch-action: none; user-select: none; }
.map.dragging { cursor: grabbing; }
.panel { background: var(--card); overflow-y: auto; min-height: 0; min-width: 0; }

/* map */
.node { cursor: pointer; }
.node rect.box { fill: var(--same); stroke: var(--line); stroke-width: 1.2; }
.node.changed rect.box { fill: var(--chg); stroke: var(--chg-edge); }
.node.added rect.box { fill: var(--add); stroke: var(--add-edge); }
.node.removed rect.box { fill: var(--del); stroke: var(--del-edge); stroke-dasharray: 5 3; }
.node.inside rect.box { stroke: var(--chg-edge); stroke-dasharray: 2 3; }
.node.feature rect.box { stroke-width: 2.2; }
.node.sel rect.box { stroke: var(--focus); stroke-width: 3; stroke-dasharray: none; }
.node.hit rect.box { stroke: var(--focus); stroke-width: 2.5; }
.proxy.hit rect.box { stroke: var(--focus); stroke-width: 2.2; stroke-dasharray: none; }
.proxy.hit text { fill: var(--ink); }
.node.hit-now rect.box, .proxy.hit-now rect.box { stroke-width: 4; filter: drop-shadow(0 0 4px var(--focus)); }
svg.map.focus g.node.hit, svg.map.focus g.proxy.hit { opacity: 1; }
#q.regex { font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
#q.bad { border-color: var(--del-edge); background: var(--del); }
.regexbtn { height: 30px; min-width: 32px; border: 1px solid var(--line); background: var(--card); border-radius: 8px; cursor: pointer;
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 12px; font-weight: 700; color: var(--muted); }
.regexbtn.on { background: var(--rust); color: var(--on-rust); border-color: var(--rust); }
.regexbtn#rxhelp.on { background: var(--rust-soft); color: var(--rust); border-color: var(--rust); }
.rxpanel { position: absolute; right: 16px; top: calc(100% + 6px); z-index: 30; width: min(460px, calc(100vw - 32px)); max-height: 70vh; overflow-y: auto;
  background: var(--card); border: 1px solid var(--line); border-radius: 10px; box-shadow: 0 8px 28px rgba(0,0,0,.18); padding: 10px 12px 12px; font-size: 13px; }
.rx-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px; }
.rx-x { border: none; background: none; cursor: pointer; color: var(--muted); font-size: 13px; }
.rx-sec { margin: 10px 0 4px; font-size: 11.5px; font-weight: 700; color: var(--muted); text-transform: uppercase; letter-spacing: .04em; }
.rx-sec .hint { text-transform: none; letter-spacing: 0; font-weight: 400; margin-left: 6px; }
.rx-list { list-style: none; margin: 0; padding: 0; }
.rx-list li { display: grid; grid-template-columns: 9.5em auto 1fr auto; gap: 8px; align-items: baseline; padding: 4px 6px; border-radius: 6px; cursor: pointer; }
.rx-list li:hover { background: var(--info); }
.rx-list .rx-name { font-weight: 600; }
.rx-list .rx-what { color: var(--muted); font-size: 12px; }
.rx-list.mine li { grid-template-columns: 9.5em 1fr auto; }
.rx-del { border: none; background: none; cursor: pointer; color: var(--muted); }
.rx-del:hover { color: var(--del-ink); }
.rx-empty { margin: 2px 6px; }
.rx-save { display: flex; gap: 6px; margin-top: 6px; }
.rx-save input { flex: 1; min-width: 0; border: 1px solid var(--line); background: var(--bg); color: var(--ink); border-radius: 6px; padding: 3px 8px; font: inherit; font-size: 12.5px; }
.rx-save button { border: 1px solid var(--line); background: var(--card); border-radius: 6px; padding: 3px 10px; cursor: pointer; font-size: 12.5px; }
.rx-save :disabled { opacity: .5; cursor: default; }
.rx-signs { margin-top: 10px; }
.rx-signs summary { cursor: pointer; font-size: 12.5px; font-weight: 600; color: var(--muted); }
.rx-signs table { border-collapse: collapse; margin-top: 4px; font-size: 12.5px; }
.rx-signs td { padding: 2px 8px 2px 0; vertical-align: baseline; }
.modline { font-size: 11.5px; }
.ctx { color: var(--muted); font-size: 11.5px; margin-left: 2px; }
.fchip .ctx { font-size: 11px; }
.sortseg { margin: 8px 0 2px; }
.chglist .why { display: block; font-size: 11.5px; color: var(--muted); margin-top: 1px; }
.chglist .first { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: .03em; color: var(--chg-ink); background: var(--chg);
  border: 1px solid var(--chg-edge); border-radius: 4px; padding: 0 4px; margin-right: 4px; }
.qcount { font-size: 12px; color: var(--muted); white-space: nowrap; font-variant-numeric: tabular-nums; }
.node text { fill: var(--ink); font-size: 13px; }
.node text.sub { fill: var(--muted); font-size: 10.5px; font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
.node text.feat { fill: var(--rust); font-size: 9.5px; font-weight: 800; letter-spacing: .08em; }
.node.removed text.lbl { text-decoration: line-through; }
.proxy { cursor: pointer; transition: opacity .38s ease; }
.pager rect.box { fill: var(--card); stroke: var(--edge); stroke-dasharray: none; }
.pager { cursor: default; }
.pg-btn { cursor: pointer; }
.pg-btn rect { fill: transparent; }
.pg-btn text { fill: var(--info-ink); font-size: 15px; font-weight: 700; }
.pg-btn:hover rect, .pg-btn:focus rect { fill: var(--info); }
.pg-btn.off { cursor: default; }
.pg-btn.off text { fill: var(--line); }
.pg-btn.off:hover rect { fill: transparent; }
.pg-label { fill: var(--muted); font-size: 11.5px; font-variant-numeric: tabular-nums; }
.pg-dot { fill: var(--line); cursor: pointer; }
.pg-dot.marked { fill: var(--chg-edge); }
.pg-dot.on { fill: var(--info-ink); }
.pg-dot.on.marked { fill: var(--chg-ink); stroke: var(--info-ink); stroke-width: 1.5; }
.edge { stroke-linejoin: round; }
.proxy rect.box { fill: var(--bg); stroke: var(--edge); stroke-width: 1.2; stroke-dasharray: 4 3; }
.proxy.changed rect.box { stroke: var(--chg-edge); }
.proxy.added rect.box { stroke: var(--add-edge); }
.proxy.removed rect.box { stroke: var(--del-edge); }
.proxy text { fill: var(--muted); font-size: 12px; }
.proxy:hover rect.box, .proxy:focus rect.box { stroke: var(--focus); stroke-dasharray: none; }
.proxy:hover text { fill: var(--ink); }
.edge.toref { stroke-dasharray: 3 3; }
svg.focus .proxy:not(.near) { opacity: .3; }
.more rect { fill: var(--bg); stroke: var(--line); }
.more text { fill: var(--muted); font-size: 11px; font-weight: 600; }
.more:hover rect { stroke: var(--focus); }
.edge { fill: none; stroke: var(--edge); stroke-width: 1.4; }
.edge.add { stroke: var(--add-edge); stroke-width: 2; }
.edge.del { stroke: var(--del-edge); stroke-width: 2; stroke-dasharray: 6 4; }
.edge.back { stroke-dasharray: 2 4; }
.order circle { fill: var(--card); stroke: var(--edge); stroke-width: 1.2; }
.order text { fill: var(--muted); font-size: 10px; font-weight: 700; }
svg.focus .order:not(.near) { opacity: .15; }
.focusbar { position: absolute; left: 12px; top: 40px; right: 12px; display: flex; gap: 10px; align-items: center; flex-wrap: wrap;
  background: var(--card); border: 1px solid var(--line); border-radius: 8px; padding: 6px 10px; font-size: 13px; width: max-content; max-width: calc(100% - 24px); }
.focusbar button { border: 1px solid var(--line); background: var(--bg); border-radius: 6px; padding: 2px 9px; cursor: pointer; font-size: 12.5px; }
#ar-same path { fill: var(--edge); } #ar-add path { fill: var(--add-edge); } #ar-del path { fill: var(--del-edge); }
svg.focus .node:not(.near) { opacity: .35; }
svg.focus .edge:not(.near) { opacity: .15; }

.legend { position: absolute; left: 12px; bottom: 12px; display: flex; gap: 12px; flex-wrap: wrap; font-size: 12px;
  background: var(--card); border: 1px solid var(--line); border-radius: 8px; padding: 6px 10px; max-width: calc(100% - 140px); }
.legend span { display: inline-flex; align-items: center; gap: 5px; white-space: nowrap; }
.sw { width: 14px; height: 10px; border-radius: 3px; border: 1.5px solid var(--line); background: var(--same); display: inline-block; }
.sw.changed { background: var(--chg); border-color: var(--chg-edge); }
.sw.added { background: var(--add); border-color: var(--add-edge); }
.sw.removed { background: var(--del); border-color: var(--del-edge); border-style: dashed; }
.sw.inside { border-color: var(--chg-edge); border-style: dotted; }
.zoom { position: absolute; right: 12px; bottom: 12px; display: flex; gap: 4px; }
.zoom button { background: var(--card); border: 1px solid var(--line); border-radius: 6px; min-width: 32px; height: 30px; cursor: pointer; }
.maphint { position: absolute; left: 50%; top: 40%; transform: translate(-50%, -50%); color: var(--muted); text-align: center; max-width: 360px; }

/* panel */
.pad { padding: 16px; }
/* A wide panel — docked below the map, or the details window — reads in two
   columns: the steps and code on the left, the facts, the glossary and the
   connections on the right. Narrow, it is one column in the same order. */
.panel { container-type: inline-size; }
@container (min-width: 980px) {
  /* Takes / gives back across the top; the code beside its variables; used
     by / uses across the bottom. */
  .fn-body { display: grid; grid-template-columns: minmax(0, 1.65fr) minmax(340px, 1fr); grid-template-areas: "facts facts" "main gloss" "conn conn"; column-gap: 28px; align-items: start; }
  .fn-facts { grid-area: facts; grid-template-columns: repeat(3, minmax(0, 1fr)); margin-bottom: 4px; }
  .fn-main { grid-area: main; min-width: 0; }
  .fn-gloss { grid-area: gloss; min-width: 0; }
  .fn-conn { grid-area: conn; min-width: 0; }
  .fn-conn .facts { grid-template-columns: 1fr 1fr; }
}
/* Small facts side by side when the panel is wide, stacked when narrow. */
.facts { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 4px 24px; align-items: start; }
.facts .rel { min-width: 0; }
.facts .ends { margin: 0; }
.facts .ends-none { font-size: 12.5px; }
.navbar { display: flex; gap: 8px; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.navbar .back { margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: left; }
.navhome { border: 1px solid var(--line); background: var(--bg); border-radius: 6px; padding: 2px 9px; font-size: 12.5px; cursor: pointer; flex: none; margin-left: auto; }
.navhome:hover { border-color: var(--focus); }
.back { background: none; border: none; color: var(--info-ink); cursor: pointer; padding: 0; margin-bottom: 10px; font-size: 13px; }
.pill { display: inline-block; font-size: 11px; font-weight: 700; padding: 1px 8px; border-radius: 999px; text-transform: uppercase; letter-spacing: .03em; }
.pill.changed { background: var(--chg); color: var(--chg-ink); }
.pill.added { background: var(--add); color: var(--add-ink); }
.pill.removed { background: var(--del); color: var(--del-ink); }
.pill.same { border: 1px solid var(--line); color: var(--muted); }
.pill.feature { background: var(--rust-soft); color: var(--rust); }
.panel h2 { margin: 8px 0 2px; font-size: 18px; line-height: 1.3; overflow-wrap: anywhere; }
.where { color: var(--muted); font-size: 12px; overflow-wrap: anywhere; }
.note { margin: 10px 0 0; padding: 8px 10px; border-radius: 8px; background: var(--chg); color: var(--chg-ink); font-size: 13px; }
.rel { margin-top: 12px; font-size: 13px; }
.rel > b { font-weight: 600; color: var(--muted); font-size: 11.5px; text-transform: uppercase; letter-spacing: .04em; display: block; margin-bottom: 4px; }
.chips { display: flex; flex-wrap: wrap; gap: 5px; }
.fchip { border: 1px solid var(--line); background: var(--bg); border-radius: 999px; padding: 1px 9px; cursor: pointer; font-size: 12.5px; }
.fchip.changed { background: var(--chg); border-color: var(--chg-edge); }
.fchip.added { background: var(--add); border-color: var(--add-edge); }
.fchip.removed { background: var(--del); border-color: var(--del-edge); }
.h3 { margin: 18px 0 8px; font-size: 11.5px; font-weight: 700; color: var(--muted); text-transform: uppercase; letter-spacing: .05em; display: flex; justify-content: space-between; align-items: center; }
.focusbtn { margin-top: 10px; }
.node { transition: transform .38s cubic-bezier(.2,.7,.3,1), opacity .38s ease; }
#edges { transition: opacity .25s ease; }
@media (prefers-reduced-motion: reduce) { .node, #edges { transition: none; } }
.h3 .hint { text-transform: none; letter-spacing: 0; font-weight: 400; font-size: 11.5px; }
.link { background: none; border: 1px solid var(--line); border-radius: 6px; padding: 2px 8px; cursor: pointer; font-size: 12px; text-transform: none; letter-spacing: 0; font-weight: 500; color: var(--ink); }

/* flowchart */
.fc { position: relative; }
.st { position: relative; display: flex; gap: 8px; align-items: baseline; margin: 0 0 6px; padding: 6px 10px;
  border-radius: 8px; border: 1px solid var(--line); background: var(--card); font-size: 13.5px; overflow-wrap: anywhere; }
.st .no { flex: none; min-width: 2.2em; color: var(--muted); font-size: 11.5px; font-weight: 700; font-variant-numeric: tabular-nums;
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
.st .ic { flex: none; width: 14px; text-align: center; color: var(--muted); font-weight: 700; }
.st .body { flex: 1; min-width: 0; }
.st .caret { flex: none; color: var(--muted); font-size: 11px; }
.st.call, .st.block { cursor: pointer; }
.st.call:hover, .st.block:hover { border-color: var(--focus); }
.st.block { background: var(--bg); border-style: dashed; }
.st.exit { border-radius: 16px; }
.st.exit.fail .ic, .st.exit.crash .ic, .st.exit.todo .ic { color: var(--del-ink); }
.st.add { background: var(--add); border-color: var(--add-edge); }
.st.del { background: var(--del); border-color: var(--del-edge); }
.st.del .t { text-decoration: line-through; text-decoration-color: var(--del-ink); }
.st.chg { background: var(--chg); border-color: var(--chg-edge); }
.st .was { display: block; margin-top: 3px; font-size: 12px; color: var(--chg-ink); }
.st .badge { font-size: 10.5px; font-weight: 700; padding: 0 6px; border-radius: 999px; margin-left: 6px; white-space: nowrap; vertical-align: 1px; }
.badge.add { background: var(--add-edge); color: #fff; } .badge.del { background: var(--del-edge); color: #fff; }
.badge.chg { background: var(--chg-edge); color: #fff; } .badge.inside { background: var(--info); color: var(--info-ink); }
.st .args { color: var(--muted); font-size: 11.5px; }
.st .q { font-size: 11.5px; color: var(--muted); font-style: italic; }
.subj { font-weight: 600; }
.toolbtn { height: 30px; border: 1px solid var(--line); background: var(--bg); color: var(--muted); border-radius: 8px; padding: 0 11px; cursor: pointer; font-size: 13px; }
.toolbtn:hover:not(.on) { color: var(--ink); }
.toolbtn.on { background: var(--rust); color: var(--on-rust); border-color: var(--rust); }
.sidetree { position: absolute; left: 0; right: 0; bottom: 0; top: 40px; overflow-y: auto; padding: 12px 14px 20px; background: var(--card); }
.side.showing-tree .map, .side.showing-tree .zoom { visibility: hidden; }
.sidetab.struct .go { font-weight: 600; }
.tree li.ti.cur { background: var(--info); outline: 1px solid var(--focus); }
.gsub { font-size: 12px; font-weight: 600; color: var(--muted); margin: 10px 0 4px; }
table.vars { border-collapse: collapse; width: 100%; font-size: 12.5px; }
/* Name, type and changes stay narrow; what it starts as takes the rest. */
table.vars th:nth-child(1) { width: 25%; } table.vars th:nth-child(2) { width: 9%; } table.vars th:nth-child(4) { width: 15%; }
table.vars th { text-align: left; font-size: 11px; color: var(--muted); font-weight: 600; padding: 3px 6px; border-bottom: 1px solid var(--line); }
table.vars td { padding: 5px 6px; border-bottom: 1px solid var(--line); vertical-align: top; overflow-wrap: anywhere; }
table.vars .vn { font-weight: 600; white-space: nowrap; }
.vline { font-size: 11px; }
.vset { white-space: normal; }
.lnref { color: var(--info-ink); cursor: pointer; text-decoration: underline dotted; font-size: 11.5px; font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
.gcards { display: grid; gap: 6px; }
.gcard { border: 1px solid var(--line); border-radius: 8px; padding: 7px 10px; cursor: pointer; background: var(--card); }
.gcard:hover { border-color: var(--focus); }
.gcard.changed { background: var(--chg); border-color: var(--chg-edge); }
.gcard.added { background: var(--add); border-color: var(--add-edge); }
.gcard.removed { background: var(--del); border-color: var(--del-edge); }
.gc-name { font-weight: 700; }
.gc-doc { font-size: 12.5px; margin-top: 2px; }
.gc-shape { font-size: 11.5px; color: var(--muted); margin-top: 3px; overflow-wrap: anywhere; }
.ovtabs { margin-bottom: 12px; }
.tree { font-size: 13px; }
.tree details > summary { list-style: none; cursor: pointer; display: flex; align-items: baseline; gap: 6px; padding: 3px 4px; border-radius: 6px; }
.tree details > summary::-webkit-details-marker { display: none; }
.tree details > summary:hover { background: var(--bg); }
.tree .tw::before { content: '▸'; display: inline-block; width: 12px; color: var(--muted); font-size: 11px; transition: transform .15s; }
.tree details[open] > summary .tw::before { transform: rotate(90deg); }
.tree .tkids { margin-left: 12px; padding-left: 6px; border-left: 1px solid var(--line); }
.tree .tfile > summary .mono { font-weight: 600; }
.tree .tn { color: var(--muted); font-size: 11px; }
.tree .cnt { margin-left: auto; font-size: 11px; font-weight: 600; color: var(--chg-ink); background: var(--chg); border-radius: 999px; padding: 0 7px; }
.tree .titems { list-style: none; margin: 0 0 4px 18px; padding: 0 0 0 6px; border-left: 1px dashed var(--line); }
.tree li.ti { display: flex; gap: 6px; align-items: baseline; padding: 2px 4px; border-radius: 6px; cursor: pointer; }
.tree li.ti:hover { background: var(--info); }
.tree li.ti .lbl { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tree li.timpl { padding: 2px 0 0 4px; color: var(--muted); font-size: 12.5px; }
.tree .nodot { width: 9px; flex: none; }
.gk.fnk { background: var(--bg); color: var(--muted); border: 1px solid var(--line); }
.tagx { font-size: 10px; font-weight: 700; color: var(--muted); border: 1px solid var(--line); border-radius: 4px; padding: 0 4px; text-transform: uppercase; }
.tagx.feat { color: var(--info-ink); border-color: var(--info-ink); }
.doc { margin: 10px 0 0; font-size: 13.5px; color: var(--ink); }
.doc p { margin: 0 0 6px; }
.params { list-style: none; margin: 0; padding: 0; display: grid; gap: 3px; }
.params .pn { font-weight: 600; }
.params .pt, .pt { color: var(--muted); }
.glink { color: var(--info-ink); text-decoration: underline dotted; text-underline-offset: 2px; cursor: pointer; }
.glink:hover { color: var(--focus); }
.gchip { border: 1px solid var(--line); background: var(--bg); border-radius: 999px; padding: 1px 9px; cursor: pointer; font-size: 12.5px; color: var(--ink); }
.gchip.changed { background: var(--chg); border-color: var(--chg-edge); }
.gchip.added { background: var(--add); border-color: var(--add-edge); }
.gchip.removed { background: var(--del); border-color: var(--del-edge); }
.gk { font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 10px; font-weight: 700; color: var(--info-ink);
  background: var(--info); border-radius: 4px; padding: 0 4px; text-transform: lowercase; }
.gfilter { width: 100%; border: 1px solid var(--line); background: var(--bg); color: var(--ink); border-radius: 8px; padding: 5px 10px; font: inherit; font-size: 13px; margin-bottom: 4px; }
.glist li { align-items: baseline; }
.glist .gdesc { color: var(--muted); font-size: 12.5px; }
table.parts { border-collapse: collapse; width: 100%; font-size: 13px; }
table.parts td { border-top: 1px solid var(--line); padding: 5px 6px; vertical-align: top; }
table.parts .pn { font-weight: 600; white-space: nowrap; }
table.parts .pd p { margin: 0; }
.code pre { margin: 0; padding: 8px 10px; font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 12px; white-space: pre; }
.stepviews { display: inline-flex; gap: 8px; align-items: center; text-transform: none; letter-spacing: 0; font-weight: 500; }
.seg.mini button { padding: 2px 9px; font-size: 12px; }
/* main path: a line with steps on it and exits off it */
.mp { padding-left: 4px; }
.mp, .mp-lane { position: relative; }
.mp::before, .mp-lane::before { content: ''; position: absolute; left: 13px; top: 4px; bottom: 4px; width: 2px; background: var(--line); }
.mp-step { position: relative; display: flex; gap: 10px; align-items: baseline; padding: 6px 8px 6px 36px; border-radius: 8px; font-size: 13.5px; overflow-wrap: anywhere; }
.mp-step .mp-ic { position: absolute; left: 3px; top: 5px; width: 22px; text-align: center; font-weight: 700; color: var(--muted); background: var(--card); line-height: 1.5; }
.mp-step .body { flex: 1; min-width: 0; }
.mp-step.opens, .mp-step.folds { cursor: pointer; }
.mp-step.folds:hover { background: var(--bg); }
.mp-step .caret { flex: none; color: var(--muted); font-size: 11px; order: 2; }
.fold.open { border-style: solid; }
.pgbar { display: flex; gap: 4px; align-items: center; flex-wrap: wrap; margin: 6px 0 4px; font-size: 12px; }
.pgbar .pg-range { color: var(--muted); margin: 0 6px; font-variant-numeric: tabular-nums; }
.pgb { border: 1px solid var(--line); background: var(--card); border-radius: 6px; min-width: 24px; padding: 1px 6px; cursor: pointer; font-size: 12px; color: var(--ink); }
.pgb:disabled { color: var(--line); cursor: default; }
.pgb.num.on { background: var(--ink); color: var(--bg); border-color: var(--ink); }
.pgb.num.marked:not(.on) { background: var(--chg); border-color: var(--chg-edge); color: var(--chg-ink); }
.folded { margin-bottom: 6px; padding-left: 8px; border-left: 2px solid var(--line); }
.mp-step.opens:hover { background: var(--bg); }
.mp-step.gate .mp-ic { color: var(--focus); }
.mp-step.gate .t, .mp-step.optional .t, .mp-step.fork .t, .mp-step.arm .t { color: var(--muted); }
.mp-step.stopper .mp-ic { color: var(--muted); }
.mp-step.loop .mp-ic { color: var(--info-ink); }
.mp-step.add { background: var(--add); } .mp-step.del { background: var(--del); } .mp-step.chg { background: var(--chg); }
.mp-step.del .t { text-decoration: line-through; }
.mp-no { flex: none; order: -1; min-width: 1.6em; font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 10.5px; color: var(--muted); opacity: .7; }
.mp-exit { position: relative; display: flex; gap: 8px; align-items: baseline; margin: 0 0 4px 36px; padding: 3px 8px 3px 10px;
  border-left: 3px solid var(--edge); border-radius: 0 7px 7px 0; background: var(--bg); font-size: 12.5px; overflow-wrap: anywhere; }
.mp-exit::before { content: ''; position: absolute; left: -25px; top: 50%; width: 22px; height: 0; border-top: 2px dotted var(--edge); }
.mp-exit .body { flex: 1; min-width: 0; }
.mp-exit .mp-x { order: -2; font-weight: 700; color: var(--muted); }
.mp-exit.err { border-left-color: var(--del-edge); }
.mp-exit.err .mp-x, .mp-exit.err .then { color: var(--del-ink); }
.mp-exit.err::before { border-top-color: var(--del-edge); }
.mp-exit.chg { background: var(--chg); }
.mp-exit.add { background: var(--add); }
.mp-exit.del { background: var(--del); }
.mp-exit.carry { border-left-style: dashed; background: none; }
.mp-more { border: 1px solid var(--line); background: var(--card); border-radius: 999px; padding: 0 7px; font-size: 11px; cursor: pointer; color: var(--muted); margin-left: 4px; }
.mp-more:hover { color: var(--ink); border-color: var(--focus); }
.mp-detail { margin: 0 0 6px 46px; }
.mp-lane { margin-left: 26px; padding-left: 4px; }
.mp-lane.loop::before { background: var(--info); }
.mp-lane.optional::before { background: repeating-linear-gradient(var(--line) 0 5px, transparent 5px 9px); }
.mp-lane.fork { margin-top: 2px; margin-bottom: 6px; }
.mp-lane.fork::before { background: var(--edge); }
.looptag { display: inline-block; font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 11px; font-weight: 700;
  padding: 0 6px; border-radius: 6px; background: var(--info); color: var(--info-ink); cursor: help; vertical-align: 1px; }
.looptag.own { margin-right: 2px; }
.stepref { font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 11.5px; font-weight: 700; color: var(--muted); }
.cut { cursor: pointer; border-radius: 3px; text-decoration: underline dotted; text-underline-offset: 3px; text-decoration-color: var(--muted); }
.cut:hover, .cut:focus-visible { color: var(--focus); text-decoration-color: currentColor; }
.cut.open { white-space: pre-wrap; }
.setnote { color: var(--muted); font-size: 12px; margin-left: 4px; }
.st.set .ic { color: var(--info-ink); }
.st .ic, .st .no, .st .then, .badge, .end { cursor: help; }
.st.call .ic, .st.block .ic { cursor: inherit; }
.signpanel { width: min(520px, calc(100vw - 32px)); }
.signpanel .key { margin: 0; border: none; background: none; }
.signpanel .key ul { padding: 4px 0 0; }
select.themesel { padding: 0 6px; background: var(--card); }
.toolbtn#signsbtn.on { background: var(--rust-soft); color: var(--rust); border-color: var(--rust); }
.key { margin-top: 14px; border: 1px solid var(--line); border-radius: 8px; background: var(--bg); font-size: 12.5px; }
.key summary { cursor: pointer; padding: 7px 10px; font-weight: 600; color: var(--muted); }
.key summary:hover { color: var(--ink); }
.key ul { list-style: none; margin: 0; padding: 0 10px 8px; display: grid; grid-template-columns: max-content 1fr; gap: 5px 12px; align-items: baseline; }
.key li { display: contents; }
.key-mark { text-align: left; white-space: nowrap; }
.key-ic { font-weight: 700; color: var(--muted); display: inline-block; min-width: 1.2em; text-align: center; }
.key-ic.error, .key-ic.crash { color: var(--del-ink); }
.key-ic.set { color: var(--info-ink); }
.key-ic.gate { color: var(--focus); }
.key-exit { display: inline-block; font-size: 11.5px; border-left: 3px solid var(--del-edge); background: var(--bg); padding: 0 6px; border-radius: 0 5px 5px 0; color: var(--del-ink); white-space: nowrap; }
.key-no { font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-weight: 700; color: var(--muted); font-size: 11.5px; }
.key-fold { color: var(--muted); font-size: 11.5px; border: 1px dashed var(--line); border-radius: 6px; padding: 0 5px; white-space: nowrap; display: inline-block; }
.key-fold.err { color: var(--del-ink); }
.key .badge { margin-left: 0; font-size: 10.5px; font-weight: 700; padding: 0 6px; border-radius: 999px; }
.st .then { color: var(--muted); font-weight: 700; margin: 0 2px; }
.st.merged.fail .then, .st.merged.crash .then { color: var(--del-ink); }
.fold.err { border-style: dashed; color: var(--del-ink); }
.fold.err b { margin-right: 2px; }
.fold .no-inline { font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 11px; color: var(--muted); margin-left: 4px; }
.kids { margin: -2px 0 8px 14px; padding: 4px 0 0 12px; border-left: 2px solid var(--line); }
.node.hover rect.box { stroke: var(--focus); stroke-width: 3; }
.ends { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 4px; }
.end { font-size: 12.5px; padding: 2px 9px; border-radius: 999px; border: 1px solid var(--line); background: var(--bg); }
.end.fail b, .end.crash b { color: var(--del-ink); }
.end.moved { background: var(--chg); border-color: var(--chg-edge); color: var(--chg-ink); font-weight: 600; }
.ends-none { margin: 0; font-size: 13px; color: var(--muted); }
.kw { color: var(--muted); }
.fold { display: block; margin: 0 0 6px; border: 1px dashed var(--line); background: none; border-radius: 8px;
  color: var(--muted); padding: 3px 10px; cursor: pointer; font-size: 12.5px; width: 100%; text-align: left; }
.fold:hover { color: var(--ink); }
.empty { color: var(--muted); font-size: 13px; }

.list { margin: 0; padding: 0; list-style: none; }
.list li { display: flex; gap: 8px; align-items: baseline; padding: 7px 0; border-top: 1px solid var(--line); cursor: pointer; }
.list li:first-child { border-top: none; }
.list li:hover .lbl { text-decoration: underline; }
.dot { width: 9px; height: 9px; border-radius: 50%; flex: none; transform: translateY(1px); }
.dot.changed { background: var(--chg-edge); } .dot.added { background: var(--add-edge); } .dot.removed { background: var(--del-edge); }
.list .lbl { flex: 1; min-width: 0; }
.list .where { flex: none; max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.summary { font-size: 14px; margin: 0 0 4px; }
.sub { color: var(--muted); font-size: 13px; margin: 0; }

/* The code is shown whole: long lines wrap (a wrapped part hangs in a
   little) and the panel scrolls, not a box of its own. */
.code { margin-top: 8px; background: var(--code); border-radius: 8px; padding: 4px 0; }
.code table { border-collapse: collapse; font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; font-size: 12px; }
.code td { padding: 0 8px; white-space: pre; vertical-align: top; }
.code table { width: 100%; table-layout: fixed; }
.code td.n { width: calc(4ch + 16px); }
.code td.s { display: flex; }
/* Each indent is its spaces wide — unless the deepest line would then take
   more than about a third of the panel: every level is scaled down alike, so the
   nesting still reads and the code keeps its room. */
.code td.s .ind { white-space: pre; flex: none; overflow: hidden; width: calc(var(--n) * min(1ch, 32cqw / var(--deepest))); }
.code td.s .txt { white-space: pre-wrap; overflow-wrap: anywhere; flex: 1 1 0; min-width: 0; padding-left: 4ch; text-indent: -4ch; }
.code td.n { color: var(--muted); text-align: right; user-select: none; }
.code .c-kw { color: var(--info-ink); font-weight: 600; }
.code .c-str { color: var(--add-ink); }
.code .c-com { color: var(--muted); font-style: italic; }
.code tr.flash td, tr.vrow.flash td { animation: flash 1.8s ease-out; }
.gcard.flash { animation: flash 1.8s ease-out; }
a.vref { color: inherit; text-decoration: underline dotted; text-underline-offset: 2px; cursor: pointer; }
a.fref { color: var(--rust); text-decoration: underline; text-decoration-color: color-mix(in srgb, var(--rust) 45%, transparent); text-underline-offset: 2px; cursor: pointer; }
a.fref:hover, a.vref:hover { text-decoration-style: solid; }
a.vref:hover { color: var(--focus); }
@keyframes flash { 0%, 40% { background: var(--chg); } 100% { background: transparent; } }
.code-note { margin: 0 0 6px; font-size: 12.5px; color: var(--muted); }
.no[data-ln], .mp-no[data-ln] { cursor: pointer; }
.no[data-ln]:hover, .mp-no[data-ln]:hover { color: var(--focus); text-decoration: underline; opacity: 1; }
.code tr.add { background: var(--add); } .code tr.del { background: var(--del); }
.code tr.add td.s::before { content: "+ "; color: var(--add-ink); }
.code tr.del td.s::before { content: "- "; color: var(--del-ink); }
.code tr.same td.s::before { content: "  "; }

@media (max-width: 860px) {
  .split.dock-right, .split.dock-left, .split.dock-bottom { grid-template-columns: 1fr; grid-template-rows: 58vh auto; grid-template-areas: "map" "panel"; }
  .gutter { display: none; }
  .panel { overflow: visible; }
  #d-left, #d-right, #d-window { display: none; }
  body { display: block; }
  .legend { max-width: calc(100% - 24px); bottom: 52px; }
  .maps { flex-direction: column; }
  .maps > .mapwrap { min-height: 50vh; }
  .mapsgutter { width: auto; height: 6px; cursor: row-resize; }
  #q { width: 150px; }
}
"##;

const JS: &str = r##"
const $ = (id) => document.getElementById(id);

// The reader's theme and font, set before anything is drawn so the page does
// not flash the defaults first. Empty means the default: the system's.
const LOOKS = { theme: 'flowlens:theme', font: 'flowlens:font' };
for (const [what, key] of Object.entries(LOOKS)) {
  let v = '';
  try { v = localStorage.getItem(key) || ''; } catch (e) {}
  if (v) document.documentElement.dataset[what] = v;
}
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const code = (s) => '<code>' + esc(s) + '</code>';
/// Text cut short with "…": a click shows it whole, another shortens it again.
const cutLink = (short, full) => '<span class="cut" data-short="' + esc(short) + '" data-full="' + esc(full) +
  '" title="Click to show it all" role="button" tabindex="0">' + esc(short) + '</span>';
/// `s` as HTML, or its first `n` characters and "…" when longer.
const cut = (s, n) => (s.length > n ? cutLink(s.slice(0, n - 1) + '…', s) : esc(s));
const plural = (n, one, many) => n + ' ' + (n === 1 ? one : many);
const SVGNS = 'http://www.w3.org/2000/svg';

const F = DATA.fns;
const KEYS = Object.keys(F);
for (const k of KEYS) F[k].key = k;
/// Project flow shows the code as it is now, compared with nothing.
let PLAIN = false;
const onMap = (k) => F[k] && !F[k].t && !(PLAIN && F[k].gone);
/// What a function calls, in the order it calls them. A call the change
/// removed stays where it used to be, after the call that preceded it.
function orderedCalls(k) {
  const out = F[k].ce.slice();
  F[k].cbe.forEach((c, i) => {
    if (out.includes(c)) return;
    const prev = F[k].cbe.slice(0, i).reverse().find((p) => out.includes(p));
    out.splice(prev === undefined ? 0 : out.indexOf(prev) + 1, 0, c);
  });
  return out.filter(onMap);
}
const CALLS = {};
const calls = (k) => CALLS[k] || (CALLS[k] = orderedCalls(k));
const callers = {};
function indexCalls() {
  for (const k in CALLS) delete CALLS[k];
  for (const k in callers) delete callers[k];
  for (const k of KEYS) for (const c of calls(k)) (callers[c] = callers[c] || []).push(k);
}
indexCalls();

/// Each function, glossary entry and feature has two faces: compared with
/// the earlier version, and the code as it is now with nothing to compare.
/// The view picks one; everything else reads the same fields either way.
const nowRows = (rows) => rows.filter((r) => r.a).map((r) => ({ m: 'same', a: r.a, b: r.a, c: nowRows(r.c) }));
let faces = null;
function setFaces(plain) {
  if (!faces) {
    faces = { fn: {}, g: {}, ft: [] };
    for (const k of KEYS) {
      const f = F[k];
      f.gone = f.s === 'removed';
      faces.fn[k] = [{ s: f.s, sig: f.sig, cb: f.cb, d: f.d, cbe: f.cbe, rows: f.rows, code: f.code },
        { s: 'same', sig: null, cb: [], d: false, cbe: f.ce, rows: nowRows(f.rows), code: f.code.filter((c) => c[2]).map((c) => ['same', c[2], c[2], c[3]]) }];
    }
    for (const k of GKEYS) {
      G[k].gone = G[k].s === 'removed';
      faces.g[k] = [{ s: G[k].s, srcb: G[k].srcb }, { s: 'same', srcb: null }];
    }
    faces.ft = DATA.features.map((ft) => [{ s: ft.s, ci: ft.ci }, { s: 'same', ci: [] }]);
  }
  PLAIN = plain;
  const i = plain ? 1 : 0;
  for (const k of KEYS) Object.assign(F[k], faces.fn[k][i]);
  for (const k of GKEYS) Object.assign(G[k], faces.g[k][i]);
  DATA.features.forEach((ft, j) => Object.assign(ft, faces.ft[j][i]));
  indexCalls();
}
const fnName = (f) => (f.o ? f.o + '::' : '') + f.n;

/// The module a function is defined in: its key without the function, and
/// without the type for a method — `crate::configuration`.
const moduleOf = (f) => f.key.split('::').slice(0, f.o ? -2 : -1).join('::') || 'crate';

/// Each file's shortest name that still tells it apart: the file name, and
/// when another file in the same crate has that name too, the folder where
/// their paths first differ — `configuration/mod.rs`, `jobs/mod.rs`; folders
/// in between that are the same collapse to `…` — `api/…/mod.rs`.
let shortFiles = null;
const shortFile = (path) => (shortFiles = shortFiles || buildShortFiles())[path];
function buildShortFiles() {
  const groups = {};
  const add = (crate, path) => ((groups[crate] = groups[crate] || new Set()).add(path));
  for (const k of KEYS) add(MULTI ? k.split('::')[0] : '', F[k].f);
  const out = {};
  for (const paths of Object.values(groups)) {
    const all = [...paths].map((p) => p.split('/'));
    for (const segs of all) {
      const name = segs[segs.length - 1];
      const twins = all.filter((o) => o !== segs && o[o.length - 1] === name);
      if (!twins.length) { out[segs.join('/')] = name; continue; }
      // Walk up from the file until this path's folder differs from every twin's.
      let up = 2;
      while (up <= segs.length && twins.some((o) => o.length >= up && o[o.length - up] === segs[segs.length - up])) up++;
      const folder = segs[segs.length - up];
      out[segs.join('/')] = folder === undefined ? segs.join('/') : up === 2 ? folder + '/' + name : folder + '/…/' + name;
    }
  }
  return out;
}

/// Where a function is defined, on its box: the file's short name and the
/// line — `mod.rs:17`, or `jobs/mod.rs:17` where another `mod.rs` exists. In
/// a workspace the crate comes first. The full path is in the box's hover
/// and in its details.
function whereShort(f, chars) {
  const text = crateOf(f) + (shortFile(f.f) || f.f.split('/').pop()) + ':' + f.ln;
  return text.length > chars ? '…' + text.slice(text.length - chars + 1) : text;
}

/// The full answer, for hovers: file and line, module, and type.
const whereLong = (f) => 'Defined in ' + f.f + ', line ' + f.ln + '\nModule ' + moduleOf(f) + (f.o ? ' · impl ' + f.o : '');
/// A function's description; one cut short opens to its whole first paragraph.
function labelHTML(f) {
  if (!f.l.endsWith('…') || !f.doc) return esc(f.l);
  return cutLink(f.l, f.doc.split(/\n\s*\n/)[0].replace(/\n/g, ' ').replace(/`/g, '').trim());
}
const affected = (k) => F[k].s !== 'same' || F[k].cb.length > 0;
/// In a workspace, keys start with the crate name; say which crate a box is in.
const MULTI = KEYS.some((k) => !k.startsWith('crate::'));
const crateOf = (f) => (MULTI ? f.key.split('::')[0] + ' · ' : '');
const fullName = (f) => crateOf(f) + (f.o ? f.o + '::' : '') + f.n;
const STATUS_WORD = { changed: 'Changed', added: 'New', removed: 'Removed', same: 'Unchanged' };

/// The direction is a per-viewer preference, so it lives in this browser
/// only; storage can be unavailable, and the page works without it.
const DIR_KEY = 'flowlens:direction';
const savedDir = (() => { try { return localStorage.getItem(DIR_KEY); } catch (e) { return null; } })();
const state = {
  mode: 'changes', selected: null, focus: null, query: '', dir: savedDir === 'v' ? 'v' : 'h',
  /// What the side space shows: 'structure', 'flow', or nothing.
  sideTab: null, structureOpen: false,
  /// Flows taken out into the side space, by the function they start at,
  /// and the one shown there.
  aside: [], asideOn: null,
};

// ------------------------------------------------------------ the map

/// How many calls deep the project overview goes before "+N" takes over.
const OVERVIEW_DEPTH = 1;

/// What the map shows. "Changes": every changed function and the paths from
/// the features down to them. "Project": each feature and the first levels of
/// what it uses, plus every change. Either way, "+N" on a box unfolds what it
/// calls, so the map only grows where the reader looks.
function visible(expanded) {
  if (state.focus) return focusView(state.focus, expanded);
  const nodes = new Set(KEYS.filter((k) => onMap(k) && affected(k)));
  if (state.mode === 'all') {
    const seen = new Set();
    let level = KEYS.filter((k) => onMap(k) && F[k].ft && calls(k).length);
    for (let d = 0; d <= OVERVIEW_DEPTH && level.length; d++) {
      const next = [];
      for (const k of level) {
        if (seen.has(k)) continue;
        seen.add(k);
        nodes.add(k);
        if (d < OVERVIEW_DEPTH) next.push(...calls(k));
      }
      level = next;
    }
  }
  // A function picked from search or the panel that is not on the map comes
  // with its path back to a feature, so it does not float on its own —
  // unless it is already in the side space.
  const sel = state.selected;
  if (sel && onMap(sel) && !nodes.has(sel) && !(side && side.placed[sel])) {
    for (const a of ancestors(sel)) nodes.add(a);
    nodes.add(sel);
  }
  // An unfold counts only while its box is on the map: once a box above it
  // folds away, what it had opened goes too. Repeated, since one unfold can
  // bring in the box of another.
  for (let grew = true; grew;) {
    grew = false;
    for (const k of expanded) {
      if (!nodes.has(k) || !onMap(k)) continue;
      for (const c of calls(k)) if (!nodes.has(c)) { nodes.add(c); grew = true; }
    }
  }
  return pruneAside(nodes);
}

/// A flow taken out into the side space leaves only its first box behind:
/// whatever the main map reaches solely through that box goes with it.
/// Functions also reached some other way stay.
// ------------------------------------------------------------ hiding
//
// The reader can hide a function from the map — a helper they do not care
// about, a branch they have read. Its box goes, and so does whatever the map
// reaches only through it; what is also reached some other way stays. The
// code is not touched: hiding is a view, kept per project in this browser,
// and every hidden function is one click from coming back.

const HIDDEN_KEY = 'flowlens:hidden:' + DATA.crate;
const hidden = new Set((() => { try { return JSON.parse(localStorage.getItem(HIDDEN_KEY)) || []; } catch (e) { return []; } })()
  .filter((k) => F[k]));
const keepHidden = () => { try { localStorage.setItem(HIDDEN_KEY, JSON.stringify([...hidden])); } catch (e) {} };

/// Drop hidden functions, and what is reachable only through them, from what
/// a map is about to draw. `keep` (a side space's own root) always stays.
function dropHidden({ nodes, edges }, keep) {
  if (!hidden.size) return { nodes, edges };
  const set = new Set(nodes);
  const into = {};
  for (const e of edges) (into[e.v] = into[e.v] || []).push(e.u);
  const out = {};
  for (const e of edges) (out[e.u] = out[e.u] || []).push(e.v);
  const gone = (k) => hidden.has(k) && k !== keep;
  const reach = new Set();
  const queue = [...set].filter((k) => !(into[k] || []).length && !gone(k));
  for (const k of queue) reach.add(k);
  while (queue.length) {
    const k = queue.shift();
    for (const c of out[k] || []) if (!gone(c) && !reach.has(c)) { reach.add(c); queue.push(c); }
  }
  // Something kept only for being selected, with nothing leading to it.
  for (const k of set) if (!gone(k) && !(into[k] || []).length) reach.add(k);
  return { nodes: [...reach], edges: edges.filter((e) => reach.has(e.u) && reach.has(e.v)) };
}

function hideFn(k) {
  if (!F[k] || hidden.has(k)) return;
  hidden.add(k);
  keepHidden();
  if (state.selected === k) state.selected = null;
  drawAll(false);
  renderHidden();
  if (state.sideTab === 'structure') renderSide();
  if (!state.selected) showOverview(); else showFunction(state.selected);
  toast('Hidden from the map: <b>' + esc(fnName(F[k])) + '</b>', 'Undo', () => restoreFn(k));
}

function restoreFn(k) {
  if (!hidden.delete(k)) return;
  keepHidden();
  drawAll(false);
  renderHidden();
  if (state.sideTab === 'structure') renderSide();
  if (state.selected) showFunction(state.selected);
}

function restoreAll() {
  if (!hidden.size) return;
  hidden.clear();
  keepHidden();
  drawAll(false);
  renderHidden();
  if (state.sideTab === 'structure') renderSide();
  if (state.selected) showFunction(state.selected);
}

/// The "N hidden" chip on the map and its list: each function with Restore,
/// and Restore all.
function renderHidden() {
  const box = $('hiddenbox'), chip = $('hiddenchip'), list = $('hiddenlist');
  box.hidden = !hidden.size;
  if (!hidden.size) { list.hidden = true; return; }
  chip.textContent = '⊘ ' + hidden.size + ' hidden';
  const keys = [...hidden].sort((a, b) => fnName(F[a]).localeCompare(fnName(F[b])));
  list.innerHTML = '<div class="hl-head"><b>Hidden from the map</b><button class="link" id="hl-all">Restore all</button></div><ul>' +
    keys.map((k) => '<li><span class="mono" title="' + esc(F[k].f + ':' + F[k].ln) + '">' + esc(fnName(F[k])) + '</span>' +
      '<button class="link" data-r="' + esc(k) + '">Restore</button></li>').join('') + '</ul>';
  list.querySelectorAll('[data-r]').forEach((b) => (b.onclick = () => restoreFn(b.dataset.r)));
  $('hl-all').onclick = restoreAll;
}

function wireHidden() {
  $('hiddenchip').onclick = () => {
    const list = $('hiddenlist');
    list.hidden = !list.hidden;
    $('hiddenchip').setAttribute('aria-expanded', String(!list.hidden));
  };
  renderHidden();
}

/// A short message at the bottom of the map, with one action.
let toastTimer = 0;
function toast(html, action, run) {
  const t = $('toast');
  t.innerHTML = '<span>' + html + '</span>' + (action ? '<button class="link" id="toast-act">' + esc(action) + '</button>' : '');
  t.hidden = false;
  if (action) $('toast-act').onclick = () => { t.hidden = true; run(); };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (t.hidden = true), 6000);
}

const isHidden = (k) => hidden.has(k);
/// A function's calls in order, without the ones hidden from the map.
const liveCalls = (k) => calls(k).filter((c) => !hidden.has(c));

function pruneAside(nodes) {
  const out = (k) => calls(k).filter((c) => nodes.has(c));
  const into = {};
  for (const k of nodes) for (const c of out(k)) (into[c] = into[c] || []).push(k);
  const aside = new Set(state.aside.filter((k) => nodes.has(k)));
  if (!aside.size) return withEdges(nodes);
  const keep = new Set();
  const queue = [...nodes].filter((k) => !(into[k] || []).length);
  for (const k of queue) keep.add(k);
  while (queue.length) {
    const k = queue.shift();
    if (aside.has(k)) continue;
    for (const c of out(k)) if (!keep.has(c)) { keep.add(c); queue.push(c); }
  }
  for (const a of aside) keep.add(a);
  return withEdges(keep);
}

/// The side space: a taken-out function, what it calls, every change below
/// it with the path to it, and whatever the reader unfolds there.
function asideView(root, expanded) {
  const nodes = new Set([root]);
  calls(root).forEach((c) => nodes.add(c));
  const seen = new Set([root]);
  const queue = [root];
  while (queue.length) {
    for (const c of calls(queue.shift())) {
      if (seen.has(c)) continue;
      seen.add(c);
      if (affected(c)) { nodes.add(c); queue.push(c); }
    }
  }
  for (const k of expanded) if (nodes.has(k)) calls(k).forEach((c) => nodes.add(c));
  // The selection, when it is somewhere in this flow, with the path to it.
  for (const k of pathWithin(root, state.selected)) nodes.add(k);
  return withEdges(nodes);
}

/// The shortest chain of calls from `root` down to `k`, both included; empty
/// when `k` is not in `root`'s flow.
function pathWithin(root, k) {
  if (!k || !onMap(k)) return [];
  const parent = { [root]: null };
  const queue = [root];
  while (queue.length) {
    const u = queue.shift();
    if (u === k) {
      const out = [];
      for (let x = k; x !== null; x = parent[x]) out.push(x);
      return out;
    }
    for (const c of calls(u)) if (!(c in parent)) { parent[c] = u; queue.push(c); }
  }
  return [];
}

/// Everything about one function and nothing else: the chain of callers that
/// leads to it from the features, the function itself, and what it calls.
/// The rest of the project collapses away.
function focusView(k, expanded) {
  const nodes = new Set([k]);
  const queue = [k];
  while (queue.length) {
    for (const c of callers[queue.shift()] || []) {
      if (onMap(c) && !nodes.has(c)) { nodes.add(c); queue.push(c); }
    }
  }
  calls(k).forEach((c) => nodes.add(c));
  for (const e of expanded) if (nodes.has(e)) calls(e).forEach((c) => nodes.add(c));
  return withEdges(nodes);
}

/// Every function on a path from a feature down to `k`.
function ancestors(k) {
  const out = new Set();
  const queue = [k];
  while (queue.length) {
    for (const c of callers[queue.shift()] || []) {
      if (onMap(c) && !out.has(c)) { out.add(c); queue.push(c); }
    }
  }
  return out;
}

function withEdges(nodes) {
  const edges = [];
  for (const u of nodes) for (const v of calls(u)) {
    if (!nodes.has(v)) continue;
    const now = F[u].ce.includes(v), before = F[u].cbe.includes(v);
    edges.push({ u, v, kind: now && before ? 'same' : now ? 'add' : 'del' });
  }
  return { nodes: [...nodes], edges };
}

const W = 216, H = 50;
/// Gaps between levels and between neighbours, per direction. Horizontal
/// levels are columns, so boxes stack by their height; vertical levels are
/// rows, so they sit side by side by their width.
const GAP = {
  h: { level: 76, beside: 16 },
  v: { level: 64, beside: 24 },
};
const along = () => (state.dir === 'h' ? W + GAP.h.level : H + GAP.v.level);
const across = () => (state.dir === 'h' ? H + GAP.h.beside : W + GAP.v.beside);

/// Height of a reference chip: a call to a function drawn elsewhere.
const PH = 30;

/// A tree, read the way the code runs. Each function is drawn once, at the
/// first place the walk reaches it — calls taken in the order they are made.
/// Every other call to it becomes a small reference chip beside the caller,
/// instead of a long arrow across the map; so does a call back up the chain.
/// Each function's calls sit side by side in call order, and the function is
/// centred over them.
///
/// Returns the boxes' positions, the chips, and the arrows to draw.
/// A box with more calls than this shows them a page at a time.
const CAP = 6;

/// A call that must always be seen: it changed, leads to a change, is
/// selected, or was taken out into the side space.
const mustShow = (k) => affected(k) || k === state.selected || state.aside.includes(k) || F[k].ft;

function layout(nodes, edges, opts = {}) {
  const boxH = opts.boxH || (() => H);
  const pages = opts.pages || new Map();
  const out = {}, inn = {};
  for (const n of nodes) { out[n] = []; inn[n] = []; }
  for (const e of edges) { out[e.u].push(e); inn[e.v].push(e); }

  const order = (a, b) => (F[b].ft - F[a].ft) || (affected(b) - affected(a)) ||
    (F[a].f < F[b].f ? -1 : F[a].f > F[b].f ? 1 : F[a].ln - F[b].ln);
  const roots = nodes.filter((n) => !inn[n].length).sort(order);
  const rest = nodes.slice().sort(order);

  // Depth-first, calls in order: the first visit of a function is its place.
  const parent = {}, depth = {}, onStack = {};
  const kids = {};
  const proxies = [];
  const visit = (n, d) => {
    depth[n] = d;
    onStack[n] = true;
    kids[n] = [];
    for (const e of out[n]) {
      if (depth[e.v] === undefined) {
        parent[e.v] = n;
        kids[n].push({ real: e.v });
        visit(e.v, d + 1);
      } else {
        const px = { id: 'px' + proxies.length, u: n, v: e.v, kind: e.kind, back: !!onStack[e.v] };
        proxies.push(px);
        kids[n].push({ proxy: px });
      }
    }
    onStack[n] = false;
  };
  const tops = [];
  for (const r of roots.concat(rest)) if (depth[r] === undefined) { tops.push(r); visit(r, 0); }

  // Too many calls under one box: show them a page at a time, in call order,
  // with a pager after them. A page holding a change is marked; the box opens
  // on the first such page, and always on the page of the selected call.
  const pagers = [];
  const folded = new Set();
  const dropped = new Set();
  const hide = (it) => {
    if (it.proxy) { dropped.add(it.proxy.id); return; }
    // A busy box on another page takes its own pager with it.
    if (it.pager) { dropped.add(it.pager.id); return; }
    folded.add(it.real);
    for (const k of kids[it.real] || []) hide(k);
  };
  const target = (it) => it.real || it.proxy.v;
  const matters = (it) => it.real ? mustShow(it.real) || (kids[it.real] || []).some((k) => k.real && affected(k.real)) : mustShow(it.proxy.v);
  for (const n of Object.keys(kids)) {
    // A box already on another page of its caller's calls is not drawn, so
    // it gets no pager of its own.
    if (folded.has(n)) continue;
    const ks = kids[n];
    if (ks.length <= CAP) continue;
    const count = Math.ceil(ks.length / CAP);
    const pageOf = (i) => Math.floor(i / CAP);
    const marked = [];
    ks.forEach((it, i) => { if (matters(it) && !marked.includes(pageOf(i))) marked.push(pageOf(i)); });
    const sel = ks.findIndex((it) => target(it) === state.selected);
    let page = pages.has(n) ? pages.get(n) : marked.length ? marked[0] : 0;
    if (sel >= 0 && pageOf(sel) !== page && opts.followSelection) page = pageOf(sel);
    page = Math.min(Math.max(page, 0), count - 1);
    const shown = ks.slice(page * CAP, page * CAP + CAP);
    ks.forEach((it) => { if (!shown.includes(it)) hide(it); });
    const pager = { id: 'pager' + pagers.length, u: n, page, count, total: ks.length,
      from: page * CAP + 1, to: page * CAP + shown.length, marked };
    pagers.push(pager);
    kids[n] = shown.concat([{ pager }]);
  }

  // Across the levels, in centres: down a column when horizontal, along a
  // row when vertical.
  const h = state.dir === 'h';
  const gap = h ? GAP.h.beside : GAP.v.beside;
  const size = (item) => (h ? (item.real ? boxH(item.real) : PH) : W);
  const keyOf = (item) => item.real || (item.proxy ? item.proxy.id : item.pager.id);
  const c = {};
  const place = (item, start) => {
    const sz = size(item);
    const ks = item.real ? kids[item.real] : [];
    const key = keyOf(item);
    if (!ks.length) { c[key] = start + sz / 2; return start + sz + gap; }
    let next = start;
    for (const k of ks) next = place(k, next);
    // Centred over its calls — but never reaching above the space it was
    // given, which a box taller than the calls under it would.
    c[key] = Math.max((c[keyOf(ks[0])] + c[keyOf(ks[ks.length - 1])]) / 2, start + sz / 2);
    return Math.max(next, c[key] + sz / 2 + gap);
  };
  let next = 0;
  for (const r of tops) next = place({ real: r }, next) + gap * 2;

  const at = (rank, centre, sz) => (h
    ? { x: rank * along(), y: centre - sz / 2 }
    : { x: centre - W / 2, y: rank * along() });
  const pos = {};
  for (const n of nodes) {
    if (folded.has(n)) continue;
    pos[n] = at(depth[n], c[n], h ? boxH(n) : W);
    pos[n].h = boxH(n);
  }
  // Nothing is drawn hanging off a box that is not drawn.
  const shownProxies = proxies.filter((px) => !dropped.has(px.id) && !folded.has(px.u));
  const shownPagers = pagers.filter((pg) => !dropped.has(pg.id) && !folded.has(pg.u));
  for (const px of shownProxies) Object.assign(px, at(depth[px.u] + 1, c[px.id], h ? PH : W));
  for (const pg of shownPagers) Object.assign(pg, at(depth[pg.u] + 1, c[pg.id], h ? PH : W));

  const arrows = [];
  for (const n of nodes) if (parent[n] && pos[n] && pos[parent[n]]) {
    const e = out[parent[n]].find((x) => x.v === n);
    arrows.push({ u: parent[n], v: n, kind: e.kind, to: pos[n], th: pos[n].h });
  }
  for (const px of shownProxies) arrows.push({ u: px.u, v: px.v, kind: px.kind, to: px, th: PH, proxy: px.id });
  for (const pg of shownPagers) arrows.push({ u: pg.u, v: null, kind: 'same', to: pg, th: PH, chip: pg.id });
  return { pos, proxies: shownProxies, pagers: shownPagers, arrows };
}

function svg(tag, attrs, parent) {
  const el = document.createElementNS(SVGNS, tag);
  for (const k in attrs) el.setAttribute(k, attrs[k]);
  if (parent) parent.appendChild(el);
  return el;
}

function clip(s, px) {
  const max = Math.floor(px / 7.1);
  return s.length > max ? s.slice(0, max - 1) + '…' : s;
}

/// Put `text` in an SVG text element, cut with "…" so it fits in `px` as
/// drawn in the font in use — not by counting characters, which a wide font
/// would overrun. `fromEnd` keeps the end (a file name and line) instead of
/// the start. Widths come from a canvas, so the map does not lay out again
/// for every box.
const measure = document.createElement('canvas').getContext('2d');
const fontOf = new Map();
function fitText(el, text, px, fromEnd) {
  el.textContent = text;
  const key = el.parentNode.getAttribute('class').split(' ')[0] + ' ' + el.getAttribute('class') + ' ' + (document.documentElement.dataset.font || '');
  let font = fontOf.get(key);
  if (!font) {
    const cs = getComputedStyle(el);
    font = cs.fontStyle + ' ' + cs.fontWeight + ' ' + cs.fontSize + ' ' + cs.fontFamily;
    fontOf.set(key, font);
  }
  measure.font = font;
  if (measure.measureText(text).width <= px) return;
  const longest = (cut, n) => {
    let lo = 0, hi = n;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (measure.measureText(cut(mid)).width <= px) lo = mid; else hi = mid - 1;
    }
    return lo;
  };
  // `Type::method`: the method tells two boxes apart, so the type is what
  // gets shorter — `BasenameLit…::matches_all` — while there is room.
  const sep = fromEnd ? -1 : text.lastIndexOf('::');
  if (sep > 0) {
    const owner = text.slice(0, sep), rest = text.slice(sep);
    const cut = (n) => owner.slice(0, n) + '…' + rest;
    const n = longest(cut, owner.length - 1);
    if (n >= 3) { el.textContent = cut(n); return; }
  }
  const cut = (n) => fromEnd ? '…' + text.slice(text.length - n) : text.slice(0, n) + '…';
  el.textContent = cut(longest(cut, text.length - 1));
}

/// Motion helps the eye follow a box to its new place; skip it for readers
/// who asked their system for less motion.
const CALM = window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches;
const MOVE_MS = 380;

/// One map on the page: the main one, or the side space. Each keeps its own
/// view, positions and unfolds; what it shows comes from `source`.
class MapView {
  constructor(wrap, source, opts = {}) {
    this.wrap = wrap;
    this.svgEl = wrap.querySelector('svg.map');
    this.vp = wrap.querySelector('.vp');
    this.gE = wrap.querySelector('.edges');
    this.gN = wrap.querySelector('.nodes');
    this.source = source;
    this.isSide = !!opts.side;
    /// `moved`: the reader has panned or zoomed, so resizing must not undo it.
    this.view = { s: 1, x: 0, y: 0, moved: false };
    this.placed = {};
    this.proxies = [];
    this.expanded = new Set();
    this.glide = 0;
    this.wire();
  }

  draw(fit) {
    const { nodes, edges } = dropHidden(this.source(this.expanded), this.isSide ? state.asideOn : null);
    this.pages = this.pages || new Map();

    const before = this.placed;
    // Turn to the selected call's page only when the selection is new, so
    // the reader can still page away from it.
    const follow = !!state.selected && state.selected !== this.followed;
    this.followed = state.selected;
    const drawn = layout(nodes, edges, { pages: this.pages, followSelection: follow });
    this.placed = drawn.pos;
    this.proxies = drawn.proxies.concat(drawn.pagers);
    // Remember the page each busy box is on, so redraws keep it.
    for (const pg of drawn.pagers) this.pages.set(pg.u, pg.page);
    const placed = this.placed, arrows = drawn.arrows;
    const animate = !CALM && Object.keys(before).length > 0;
    const gE = this.gE, gN = this.gN;
    gE.innerHTML = '';
    gN.innerHTML = '';
    // Arrows are redrawn in their final place; they fade in once the boxes
    // have arrived rather than stretching mid-move.
    if (animate) {
      gE.style.transition = 'none';
      gE.style.opacity = 0;
      setTimeout(() => { gE.style.transition = ''; gE.style.opacity = 1; }, MOVE_MS);
    }

    for (const e of arrows) {
      // Comb arrows: one trunk from the caller, a short straight branch to
      // each call. They stay apart however many calls there are.
      const a = placed[e.u], b = e.to, r = 6;
      let d, badge;
      if (state.dir === 'h') {
        const x1 = a.x + W, y1 = a.y + a.h / 2, x2 = b.x - 2, y2 = b.y + e.th / 2, tx = (x1 + b.x) / 2;
        const dy = y2 - y1, sy = Math.sign(dy), rr = Math.min(r, Math.abs(dy) / 2);
        d = 'M' + x1 + ',' + y1 + ' H' + (tx - rr) + (rr ? ' Q' + tx + ',' + y1 + ' ' + tx + ',' + (y1 + sy * rr) : '') +
          ' V' + (y2 - sy * rr) + (rr ? ' Q' + tx + ',' + y2 + ' ' + (tx + rr) + ',' + y2 : '') + ' H' + x2;
        badge = { x: tx + 13, y: y2 };
      } else {
        const x1 = a.x + W / 2, y1 = a.y + a.h, x2 = b.x + W / 2, y2 = b.y - 2, ty = (y1 + b.y) / 2;
        const dx = x2 - x1, sx = Math.sign(dx), rr = Math.min(r, Math.abs(dx) / 2);
        d = 'M' + x1 + ',' + y1 + ' V' + (ty - rr) + (rr ? ' Q' + x1 + ',' + ty + ' ' + (x1 + sx * rr) + ',' + ty : '') +
          ' H' + (x2 - sx * rr) + (rr ? ' Q' + x2 + ',' + ty + ' ' + x2 + ',' + (ty + rr) : '') + ' V' + y2;
        badge = { x: x2, y: ty + 13 };
      }
      const path = svg('path', { d, class: 'edge ' + e.kind + (e.proxy || e.chip ? ' toref' : ''), 'marker-end': 'url(#ar-' + e.kind + ')' }, gE);
      path.dataset.u = e.u;
      if (e.v) path.dataset.v = e.v;

      // Number each call in the order it is made, when there is an order
      // to tell: 1 runs first.
      // Numbered among the calls still on the map: hiding one closes the gap.
      const live = liveCalls(e.u);
      if (e.v && live.length > 1) {
        const n = svg('g', { class: 'order', transform: 'translate(' + badge.x + ',' + badge.y + ')' }, gE);
        n.dataset.u = e.u;
        n.dataset.v = e.v;
        svg('circle', { r: 8 }, n);
        svg('text', { y: 3.5, 'text-anchor': 'middle' }, n).textContent = live.indexOf(e.v) + 1;
        svg('title', {}, n).textContent = 'Call ' + (live.indexOf(e.v) + 1) + ' of ' + live.length + ' made by ' + F[e.u].l +
          (live.length < calls(e.u).length ? ' (not counting ' + plural(calls(e.u).length - live.length, 'hidden one', 'hidden ones') + ')' : '');
      }
    }

    // Pagers: a busy box's calls, a page at a time.
    for (const pg of drawn.pagers) {
      const g = svg('g', { class: 'proxy pager' }, gN);
      g.dataset.u = pg.u;
      g.style.transform = 'translate(' + pg.x + 'px,' + pg.y + 'px)';
      svg('rect', { class: 'box', width: W, height: PH, rx: PH / 2 }, g);
      const turn = (to) => {
        if (to < 0 || to >= pg.count) return;
        this.pages.set(pg.u, to);
        this.draw(false);
      };
      const button = (x, label, to, tip) => {
        const b = svg('g', { class: 'pg-btn' + (to < 0 || to >= pg.count ? ' off' : ''), tabindex: 0, transform: 'translate(' + x + ',0)' }, g);
        svg('rect', { width: 26, height: PH, rx: PH / 2 }, b);
        svg('text', { x: 13, y: 20, 'text-anchor': 'middle' }, b).textContent = label;
        svg('title', {}, b).textContent = tip;
        b.addEventListener('click', (ev) => { ev.stopPropagation(); turn(to); });
        b.addEventListener('keydown', (ev) => { if (ev.key === 'Enter') turn(to); });
      };
      button(0, '‹', pg.page - 1, 'Previous calls');
      button(W - 26, '›', pg.page + 1, 'Next calls');
      svg('text', { class: 'pg-label', x: 34, y: 19.5 }, g).textContent = 'calls ' + pg.from + '–' + pg.to + ' of ' + pg.total;
      // One dot per page; amber where a page holds a change. Click to go.
      const dots = Math.min(pg.count, 8), x0 = W - 34 - dots * 11;
      for (let i = 0; i < dots; i++) {
        const d = svg('circle', { class: 'pg-dot' + (i === pg.page ? ' on' : '') + (pg.marked.includes(i) ? ' marked' : ''),
          cx: x0 + i * 11 + 5, cy: PH / 2, r: 3.6 }, g);
        svg('title', {}, d).textContent = 'Calls ' + (i * CAP + 1) + '–' + Math.min((i + 1) * CAP, pg.total) +
          (pg.marked.includes(i) ? ' — something here changed' : '');
        d.addEventListener('click', (ev) => { ev.stopPropagation(); turn(i); });
      }
    }

    // Reference chips: this call goes to a function whose box is elsewhere.
    for (const px of drawn.proxies) {
      const f = F[px.v];
      const g = svg('g', { class: 'proxy ' + f.s + (px.back ? ' back' : ''), tabindex: 0 }, gN);
      g.dataset.k = px.v;
      g.dataset.u = px.u;
      g.style.transform = 'translate(' + px.x + 'px,' + px.y + 'px)';
      if (animate) { g.style.opacity = 0; requestAnimationFrame(() => requestAnimationFrame(() => (g.style.opacity = ''))); }
      svg('title', {}, g).textContent = px.back
        ? F[px.u].l + ' calls back to ' + f.l + ', which is above it. Click to go there.'
        : F[px.u].l + ' also calls ' + f.l + '. Its box is drawn where it is first called. Click to go there.';
      svg('rect', { class: 'box', width: W, height: PH, rx: PH / 2 }, g);
      fitText(svg('text', { class: 'lbl', x: 12, y: 19.5 }, g), (px.back ? '↺ calls back to ' : '↪ ') + fnName(f), W - 24);
      const real = () => this.gN.querySelector('.node[data-k="' + CSS.escape(px.v) + '"]');
      g.addEventListener('mouseenter', () => real()?.classList.add('hover'));
      g.addEventListener('mouseleave', () => real()?.classList.remove('hover'));
      const go = () => { real()?.classList.remove('hover'); select(px.v, false, this); this.centerOn(px.v); };
      g.addEventListener('click', go);
      g.addEventListener('keydown', (ev) => { if (ev.key === 'Enter') go(); });
    }

    const shown = new Set(nodes);
    for (const k of nodes) {
      const f = F[k], p = placed[k];
      // On another page of its caller's calls.
      if (!p) continue;
      const takenOut = !this.isSide && state.aside.includes(k);
      const cls = ['node', f.s];
      if (f.s === 'same' && f.cb.length) cls.push('inside');
      if (f.ft) cls.push('feature');
      if (takenOut) cls.push('aside');
      if (state.selected === k) cls.push('sel');
      if (state.query && matches(k, state.query)) cls.push('hit');
      const g = svg('g', { class: cls.join(' '), tabindex: 0 }, gN);
      g.dataset.k = k;
      const at = (q) => 'translate(' + q.x + 'px,' + q.y + 'px)';
      if (animate) {
        // A box that was already on the map glides from where it was; a new
        // one grows out of the box that calls it.
        const from = before[k] || before[(callers[k] || []).find((c) => before[c])] || p;
        g.style.transform = at(from);
        if (!before[k]) g.style.opacity = 0;
        // Clearing the opacity hands it back to the stylesheet, which dims
        // boxes away from the selection.
        requestAnimationFrame(() => requestAnimationFrame(() => { g.style.transform = at(p); g.style.opacity = ''; }));
      } else {
        g.style.transform = at(p);
      }
      svg('title', {}, g).textContent = fullName(f) + '\n' + (f.doc ? f.doc.split(/\n\s*\n/)[0].replace(/\n/g, ' ').replace(/`/g, '') : f.l) +
        '\n' + whereLong(f) + (PLAIN || !DATA.history ? '' : '\n' + STATUS_WORD[f.s]) +
        (takenOut ? '\nIts flow is in the side space.' : '');
      const bh = p.h || H;
      svg('rect', { class: 'box', width: W, height: bh, rx: 9 }, g);
      if (f.ft) svg('text', { class: 'feat', x: 12, y: -5 }, g).textContent = 'FEATURE';
      fitText(svg('text', { class: 'lbl', x: 12, y: 21 }, g), fnName(f), W - 24);
      // Where it is defined: file and line, clear of the +N chip; hover for
      // the full path.
      fitText(svg('text', { class: 'sub', x: 12, y: 38 }, g), whereShort(f, Infinity), W - 62, true);
      if (takenOut) {
        // Marker for a flow that now lives in the side space.
        const m = svg('g', { class: 'asidechip', transform: 'translate(' + (W - 104) + ',' + (bh - 22) + ')' }, g);
        svg('rect', { width: 98, height: 17, rx: 8.5 }, m);
        svg('text', { x: 49, y: 12.5, 'text-anchor': 'middle' }, m).textContent = '⇱ in side space';
        svg('title', {}, m).textContent = 'Show its flow in the side space';
        m.addEventListener('click', (ev) => { ev.stopPropagation(); showAside(k); });
      } else {
        const hidden = calls(k).filter((c) => !shown.has(c) && !isHidden(c)).length;
        const open = this.expanded.has(k);
        if (hidden || open) {
          const m = svg('g', { class: 'more', transform: 'translate(' + (W - 44) + ',' + (bh - 22) + ')' }, g);
          svg('rect', { width: 38, height: 17, rx: 8.5 }, m);
          svg('text', { x: 19, y: 12.5, 'text-anchor': 'middle' }, m).textContent = open && !hidden ? '−' : '+' + hidden;
          svg('title', {}, m).textContent = open && !hidden ? 'Fold its calls back' : 'Show ' + plural(hidden, 'more function', 'more functions') + ' it calls';
          m.addEventListener('click', (ev) => {
            ev.stopPropagation();
            if (open && !hidden) {
              // Folding a box folds everything opened below it, so opening it
              // again starts from its own calls, not the old unfolds. A
              // selection down there moves up to this box: it would otherwise
              // hold its path open.
              this.expanded.delete(k);
              const below = new Set(), queue = [k];
              while (queue.length) for (const c of calls(queue.shift())) if (!below.has(c)) { below.add(c); queue.push(c); }
              const was = state.selected;
              const moveUp = was && was !== k && below.has(was);
              if (moveUp) state.selected = k;
              const still = new Set(this.source(this.expanded).nodes);
              for (const e of [...this.expanded]) if (!still.has(e)) this.expanded.delete(e);
              if (moveUp) { state.selected = was; select(k, false, this); }
            } else this.expanded.add(k);
            this.draw(false);
          });
        }
      }
      // Take this box's flow out into the side space, from the main map.
      if (!this.isSide && !takenOut && calls(k).length) {
        const t = svg('g', { class: 'takeout', transform: 'translate(' + (W - 26) + ',4)' }, g);
        svg('rect', { width: 22, height: 18, rx: 5 }, t);
        svg('text', { x: 11, y: 13.5, 'text-anchor': 'middle' }, t).textContent = '⇱';
        svg('title', {}, t).textContent = 'Take this flow out into the side space';
        t.addEventListener('click', (ev) => { ev.stopPropagation(); takeOut(k); });
      }
      // Hide this function from the map; it comes back from "N hidden".
      if (!(this.isSide && k === state.asideOn)) {
        const x = svg('g', { class: 'takeout hidebtn', transform: 'translate(' + (W - (!this.isSide && !takenOut && calls(k).length ? 52 : 26)) + ',4)' }, g);
        svg('rect', { width: 22, height: 18, rx: 5 }, x);
        svg('text', { x: 11, y: 13.5, 'text-anchor': 'middle' }, x).textContent = '✕';
        svg('title', {}, x).textContent = 'Hide from the map (Delete) — restore it from “hidden”';
        x.addEventListener('click', (ev) => { ev.stopPropagation(); hideFn(k); });
      }
      // Picking a box also unfolds what it calls, as its +N would; − folds
      // them back.
      const pick = () => {
        const unfold = !takenOut && calls(k).some((c) => !shown.has(c) && !isHidden(c));
        select(k, false, this);
        // A collapsed map already shows the picked box with what it calls.
        if (unfold && state.focus !== k && !this.expanded.has(k)) {
          this.expanded.add(k);
          this.draw(false);
          this.reveal(k);
        }
      };
      g.addEventListener('click', pick);
      g.addEventListener('keydown', (ev) => { if (ev.key === 'Enter') pick(); });
    }

    this.focusNear();
    // A redraw replaces the boxes; mark the search's places again.
    if (state.query) this.svgEl.querySelectorAll('.node[data-k], .proxy[data-k]').forEach((el) => {
      if (!el.classList.contains('pager') && matches(el.dataset.k, state.query)) el.classList.add('hit');
    });
    if (!this.isSide) {
      const bar = $('focusbar');
      bar.hidden = !state.focus;
      if (state.focus) {
        bar.innerHTML = '<span>Showing only <b>' + esc(F[state.focus].l) + '</b>, what leads to it, and what it calls, in order.</span>' +
          '<button id="unfocus">Show all</button>';
        $('unfocus').onclick = () => setFocus(null);
      }
      const hint = $('maphint');
      hint.textContent = nodes.length ? '' : state.mode === 'changes'
        ? 'Nothing the program does has changed. Switch to “Project flow” to see the map.'
        : 'No functions to show.';
    }
    if (fit) this.fit(animate);
    else this.apply();
  }

  /// Dim everything but the selected box, what it calls, and the path that
  /// leads to it from a feature.
  focusNear() {
    const s = state.selected, map = this.svgEl;
    map.classList.toggle('focus', !!(s && this.placed[s]));
    map.querySelectorAll('.node.sel').forEach((n) => n.classList.remove('sel'));
    if (!s) return;
    map.querySelector('.node[data-k="' + CSS.escape(s) + '"]')?.classList.add('sel');
    const path = ancestors(s);
    map.querySelectorAll('.node').forEach((n) => {
      const k = n.dataset.k;
      n.classList.toggle('near', k === s || calls(s).includes(k) || path.has(k));
    });
    map.querySelectorAll('.proxy').forEach((n) => n.classList.toggle('near', n.dataset.u === s || n.dataset.k === s));
    map.querySelectorAll('.edge, .order').forEach((e) => e.classList.toggle('near',
      e.dataset.u === s || (path.has(e.dataset.u) && (path.has(e.dataset.v) || e.dataset.v === s))));
  }

  bounds() {
    const ps = Object.values(this.placed).concat(this.proxies);
    if (!ps.length) return { x: 0, y: 0, w: 1, h: 1 };
    const x0 = Math.min(...ps.map((p) => p.x)), y0 = Math.min(...ps.map((p) => p.y)) - 16;
    const x1 = Math.max(...ps.map((p) => p.x)) + W, y1 = Math.max(...ps.map((p) => p.y + (p.h || H))) + 40;
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
  }

  apply() {
    const v = this.view;
    this.vp.setAttribute('transform', 'translate(' + v.x + ',' + v.y + ') scale(' + v.s + ')');
  }

  /// Move the view to `to` ({s, x, y}), gliding when `animate`.
  move(to, animate) {
    const view = this.view, from = { s: view.s, x: view.x, y: view.y };
    const id = ++this.glide;
    if (!animate || CALM) { Object.assign(view, to); this.apply(); return; }
    const t0 = performance.now();
    const ease = (t) => 1 - Math.pow(1 - t, 3);
    const step = (now) => {
      if (id !== this.glide) return;
      const t = Math.min(1, (now - t0) / MOVE_MS), e = ease(t);
      view.s = from.s + (to.s - from.s) * e;
      view.x = from.x + (to.x - from.x) * e;
      view.y = from.y + (to.y - from.y) * e;
      this.apply();
      if (t < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  fit(animate) {
    const r = this.svgEl.getBoundingClientRect(), b = this.bounds(), pad = 40;
    // Leave room for whatever sits on top of the map, and the legend below.
    const top = this.isSide ? 40 : $('focusbar').hidden ? 24 : 72, bottom = 40;
    const h = r.height - pad * 2 - top - bottom;
    const s = Math.max(Math.min((r.width - pad * 2) / b.w, h / b.h, 1.1), 0.15);
    this.view.moved = false;
    this.move({ s, x: (r.width - b.w * s) / 2 - b.x * s, y: pad + top + (h - b.h * s) / 2 - b.y * s }, animate);
  }

  /// Centre the view on one element of the map — a box or a ↪ chip — read
  /// from where it was drawn.
  centerOnEl(el) {
    const m = /translate\(([-\d.]+)px,\s*([-\d.]+)px\)/.exec(el.style.transform || '');
    if (!m) return;
    const x = +m[1], y = +m[2];
    const h = el.classList.contains('proxy') ? PH : (this.placed[el.dataset.k] && this.placed[el.dataset.k].h) || H;
    const f = this.free();
    const s = Math.max(this.view.s, 0.8);
    this.view.moved = true;
    this.move({ s, x: (f.x0 + f.x1) / 2 - (x + W / 2) * s, y: (f.y0 + f.y1) / 2 - (y + h / 2) * s }, true);
  }

  centerOn(k) {
    const p = this.placed[k];
    if (!p) return;
    const f = this.free();
    const s = Math.max(this.view.s, 0.8);
    // Placed on purpose: a later refit (a resize) keeps it rather than
    // snapping back to the whole map.
    this.view.moved = true;
    this.move({ s, x: (f.x0 + f.x1) / 2 - (p.x + W / 2) * s, y: (f.y0 + f.y1) / 2 - (p.y + H / 2) * s }, true);
  }

  /// Bring a box into view only if it is not already comfortably visible,
  /// keeping the zoom: the reader's view is theirs.
  /// The part of the map the reader can see: all of it, less what the
  /// details box over it covers.
  free() {
    const r = this.svgEl.getBoundingClientRect();
    const f = { x0: 0, y0: 0, x1: r.width, y1: r.height };
    const split = document.querySelector('.split');
    if (!split.classList.contains('closed') || !split.classList.contains('modal-on')) return f;
    const b = $('panel').getBoundingClientRect();
    const g = { ...f };
    if (dock === 'bottom') g.y1 = Math.min(f.y1, b.top - r.top);
    else if (dock === 'left') g.x0 = Math.max(0, b.right - r.left);
    else g.x1 = Math.min(f.x1, b.left - r.left);
    // A map almost wholly under the box keeps its own middle.
    return g.x1 - g.x0 < 200 || g.y1 - g.y0 < 120 ? f : g;
  }

  reveal(k) {
    const p = this.placed[k];
    if (!p) return;
    const v = this.view, f = this.free(), m = 60;
    const x = p.x * v.s + v.x, y = p.y * v.s + v.y;
    const inside = x > f.x0 + m && y > f.y0 + m && x + W * v.s < f.x1 - m && y + H * v.s < f.y1 - m;
    if (!inside) this.centerOn(k);
  }

  zoomAt(factor, cx, cy) {
    const v = this.view;
    v.moved = true;
    const s = Math.min(3, Math.max(0.1, v.s * factor));
    v.x = cx - (cx - v.x) * (s / v.s);
    v.y = cy - (cy - v.y) * (s / v.s);
    v.s = s;
    this.apply();
  }

  wire() {
    const map = this.svgEl, v = this.view;
    let drag = null;
    map.addEventListener('pointerdown', (e) => {
      if (e.target.closest('.node, .proxy')) return;
      drag = { x: e.clientX, y: e.clientY, vx: v.x, vy: v.y, moved: false };
      map.setPointerCapture(e.pointerId);
      map.classList.add('dragging');
    });
    map.addEventListener('pointermove', (e) => {
      if (!drag) return;
      v.moved = true;
      v.x = drag.vx + e.clientX - drag.x;
      v.y = drag.vy + e.clientY - drag.y;
      drag.moved = drag.moved || Math.abs(e.clientX - drag.x) + Math.abs(e.clientY - drag.y) > 4;
      this.apply();
    });
    map.addEventListener('pointerup', (e) => {
      if (drag && !drag.moved && !e.target.closest('.node, .proxy')) select(null, false);
      drag = null;
      map.classList.remove('dragging');
    });
    map.addEventListener('wheel', (e) => {
      e.preventDefault();
      const r = map.getBoundingClientRect();
      this.zoomAt(Math.exp(-e.deltaY * 0.0015), e.clientX - r.left, e.clientY - r.top);
    }, { passive: false });
    const mid = () => { const r = map.getBoundingClientRect(); return [r.width / 2, r.height / 2]; };
    this.wrap.querySelector('.z-in').onclick = () => this.zoomAt(1.25, ...mid());
    this.wrap.querySelector('.z-out').onclick = () => this.zoomAt(0.8, ...mid());
    this.wrap.querySelector('.z-fit').onclick = () => this.fit(true);
    // The map's area changes with the window, the docked details, the
    // divider and the side space. Refit while the reader has not moved the
    // view themselves.
    new ResizeObserver(() => {
      if (this.wrap.hidden) return;
      v.moved ? this.apply() : this.fit(false);
    }).observe(this.wrap);
  }
}

let main = null;
let side = null;

/// A taken-out flow is on show in the side space (not the structure).
const flowShown = () => !!(state.asideOn && state.sideTab === 'flow');

/// Redraw both maps: the main one, and the side space's flow when it shows.
function drawAll(fit) {
  main.draw(fit);
  if (side && flowShown()) side.draw(fit);
}

// ------------------------------------------------------------ side space
//
// The side space beside the map holds tabs: the project's structure, and any
// flows taken out of the main map. One shows at a time.

/// Take the flow starting at `k` out of the main map into the side space.
function takeOut(k) {
  if (!state.aside.includes(k)) state.aside.push(k);
  showAside(k);
  main.draw(false);
}

/// Show one taken-out flow in the side space.
function showAside(k) {
  const changed = state.asideOn !== k;
  state.asideOn = k;
  state.sideTab = 'flow';
  if (changed) side.expanded = new Set();
  openSide();
}

/// Show the project's structure in the side space.
function showStructure() {
  state.structureOpen = true;
  state.sideTab = 'structure';
  openSide();
}

function openSide() {
  $('sidewrap').hidden = false;
  renderSide();
  // The side space appearing changes the main map's size too.
  requestAnimationFrame(() => { if (!main.view.moved) main.fit(true); });
}

/// Draw whichever tab is on: the structure tree, or a flow's map.
function renderSide() {
  renderTabs();
  const wrap = $('sidewrap'), tree = $('sidetree'), structure = state.sideTab === 'structure';
  wrap.classList.toggle('showing-tree', structure);
  tree.hidden = !structure;
  if (structure) {
    const scroll = tree.scrollTop;
    tree.innerHTML = structureHTML();
    wireStructure(tree);
    tree.scrollTop = scroll;
  } else {
    requestAnimationFrame(() => side.draw(true));
  }
}

/// Close the side space when nothing is left in it, else show what is.
function settleSide() {
  if (state.sideTab === 'flow' && !state.asideOn) state.sideTab = state.structureOpen ? 'structure' : null;
  if (state.sideTab === 'structure' && !state.structureOpen) state.sideTab = state.asideOn ? 'flow' : null;
  if (!state.sideTab) {
    $('sidewrap').hidden = true;
    renderTabs();
  } else {
    renderSide();
  }
  requestAnimationFrame(() => { if (!main.view.moved) main.fit(true); });
}

/// Put a flow back into the main map.
function putBack(k) {
  state.aside = state.aside.filter((x) => x !== k);
  if (state.asideOn === k) state.asideOn = state.aside[state.aside.length - 1] || null;
  main.draw(false);
  settleSide();
}

function closeStructure() {
  state.structureOpen = false;
  settleSide();
}

function renderTabs() {
  const box = $('sidetabs');
  const tab = (on, cls, attrs, label, title, closeTitle) => '<span class="sidetab ' + cls + (on ? ' on' : '') + '"' + attrs + '>' +
    '<button class="go" title="' + esc(title) + '">' + label + '</button>' +
    '<button class="x" title="' + esc(closeTitle) + '" aria-label="Close">✕</button></span>';
  box.innerHTML = '<span class="lead" title="The structure, and flows taken out of the main map">Side space</span>' +
    (state.structureOpen ? tab(state.sideTab === 'structure', 'struct', '', '▤ Structure', 'Folders, files and what each defines', 'Close the structure') : '') +
    state.aside.map((k) => tab(state.sideTab === 'flow' && k === state.asideOn, 'flow', ' data-k="' + esc(k) + '"', esc(clip(F[k].l, 180)),
      fullName(F[k]), 'Put this flow back into the main map')).join('');
  const st = box.querySelector('.sidetab.struct');
  if (st) {
    st.querySelector('.go').onclick = showStructure;
    st.querySelector('.x').onclick = closeStructure;
  }
  box.querySelectorAll('.sidetab.flow').forEach((t) => {
    t.querySelector('.go').onclick = () => showAside(t.dataset.k);
    t.querySelector('.x').onclick = () => putBack(t.dataset.k);
  });
  $('structbtn').classList.toggle('on', !!state.structureOpen && !$('sidewrap').hidden && state.sideTab === 'structure');
  $('mapsgutter').hidden = $('sidewrap').hidden;
}

/// The bar between the main map and the side space: drag to share the room,
/// double-click to split it evenly again. Kept in this browser.
const SIDE_KEY = 'flowlens:side-size';
function wireMapsGutter() {
  const maps = $('maps'), sideEl = $('sidewrap'), bar = $('mapsgutter');
  const column = () => getComputedStyle(maps).flexDirection === 'column';
  const apply = (px) => (sideEl.style.flex = px ? '0 0 ' + px + 'px' : '');
  const saved = store.get(SIDE_KEY);
  if (saved && saved.px) apply(saved.px);
  let drag = false;
  bar.addEventListener('pointerdown', (e) => { drag = true; bar.setPointerCapture(e.pointerId); bar.classList.add('drag'); });
  bar.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const r = maps.getBoundingClientRect();
    const px = column()
      ? Math.min(Math.max(r.bottom - e.clientY, 160), r.height - 160)
      : Math.min(Math.max(r.right - e.clientX, 240), r.width - 240);
    apply(Math.round(px));
  });
  bar.addEventListener('pointerup', () => {
    drag = false;
    bar.classList.remove('drag');
    const px = parseInt(sideEl.style.flex.split(' ')[2], 10);
    store.set(SIDE_KEY, { px: px || null });
  });
  bar.addEventListener('dblclick', () => { apply(null); store.set(SIDE_KEY, { px: null }); });
}

// ------------------------------------------------------------ wording
//
// Conditions and stops are put in words where the code follows a pattern a
// person would describe the same way every time: a call that fails, a value
// that is missing, an error passed on, an error with a message. Anything else
// is quoted as code rather than guessed at.

/// What words like "the loop", "that" and "otherwise" point at, kept by the
/// flowchart as it renders: the loops around the current step (innermost
/// last), the step "that" refers to, and the conditions an "otherwise" is the
/// alternative to.
const ref = { loops: [], that: null, chain: [] };
const stepRef = (no) => '<span class="stepref" title="Step ' + no + '">' + no + '</span>';

/// A loop's name — L1, L1.2 — as a badge that says on hover which loop it is.
function loopTag(l) {
  return '<span class="looptag" title="' + esc(l.title) + '">' + l.id + '</span>';
}

/// `break` and `continue`, saying which loop they act on: "Leave loop L1",
/// "Skip to the next `order` in L1", "Go round L1.1 again".
function loopExit(s) {
  const up = parseInt(s.d, 10) || 0;
  const l = ref.loops[ref.loops.length - 1 - up];
  if (s.e === 'break') return l ? 'Leave loop ' + loopTag(l) : 'Leave the loop';
  if (!l) return 'Skip to the next round';
  return l.each ? 'Skip to the next ' + l.each + ' in ' + loopTag(l) : 'Go round ' + loopTag(l) + ' again';
}

/// Names for a function's loops, counted among loops alone: L1 and L2 at the
/// top, L1.1 and L1.2 for loops inside L1, however deep the steps between.
function loopNames(rows, parent = '', map = new Map(), counter = { n: 0 }) {
  for (const r of rows) {
    const s = r.a || r.b;
    if (s.t === 'block' && ['for', 'while', 'loop'].includes(s.b)) {
      const id = parent ? parent + '.' + ++counter.n : 'L' + ++counter.n;
      map.set(r, id);
      loopNames(r.c, id, map, { n: 0 });
    } else {
      loopNames(r.c, parent, map, counter);
    }
  }
  return map;
}

/// The first string literal in some code, `{}` placeholders shown as `…`.
function message(codeText) {
  const m = /"((?:[^"\\]|\\.)*)"/.exec(codeText || '');
  if (!m) return null;
  const t = m[1].replace(/\{[^}]*\}/g, '…').replace(/\\"/g, '"').trim();
  return t ? '“' + cut(t, 80) + '”' : null;
}

/// `Err(e)`, `Err(e.to_string())`, `Err(err.into())`: the error is passed on
/// as it came, with nothing of its own to say.
const passesOn = (inner) => /^[a-z_][a-z0-9_]*(\.(to_string|into|clone)\(\))?$/i.test(inner.trim());

function exitText(s) {
  const d = s.d || '';
  switch (s.e) {
    case 'return': {
      if (!d) return 'Stop here';
      if (/^Ok\s*\(\s*\(\s*\)\s*\)$/.test(d)) return 'Finish early';
      const ok = /^Ok\s*\((.*)\)$/.exec(d);
      if (ok) return 'Finish early with ' + code(ok[1]);
      const msg = message(d);
      return 'Stop and give back ' + (msg || quoted(d));
    }
    case 'fail': {
      const err = /^Err\s*\((.*)\)$/.exec(d);
      if (!err) return 'Stop with an error if ' + quoted(d) + ' fails';
      if (passesOn(err[1])) return 'Stop and pass the error on';
      const msg = message(err[1]);
      return 'Stop with error ' + (msg || quoted(err[1]));
    }
    case 'crash': {
      const u = /^(.*)\.(unwrap|expect)\((.*)\)$/.exec(d);
      if (u) return 'Crash if ' + code(u[1]) + ' fails' + (message(u[3]) ? ' — ' + message(u[3]) : '');
      if (/^assert/.test(d)) return 'Crash unless ' + code(d.replace(/^assert\w*!\((.*)\)$/, '$1'));
      return 'Crash' + (message(d) ? ': ' + message(d) : d ? ' ' + code(d) : '');
    }
    case 'todo': return 'Not built yet — crashes if reached';
    case 'exit': return 'Quit the program' + (d ? ' ' + code(d) : '');
    case 'break':
    case 'continue': return loopExit(s);
  }
  return esc(s.e);
}

/// What a pattern is checked against, in words: "that" when it is the call
/// just before, the called function's label when it is another call, the
/// code otherwise.
function subjectOf(s, prevCall) {
  // The call just before: said by name, with its step number.
  if (s.oc && s.oc === prevCall) {
    const name = F[s.oc] ? '<span class="subj">' + labelHTML(F[s.oc]) + '</span>' : '';
    const no = ref.that ? '<span class="stepref" title="The call in step ' + ref.that + '">step ' + ref.that + '</span>' : '';
    return { html: name && no ? name + ' (' + no + ')' : name || no, call: true };
  }
  if (s.oc && F[s.oc]) return { html: '<span class="subj">' + labelHTML(F[s.oc]) + '</span>', call: true };
  return { html: quoted(s.on), call: false };
}

/// Option and Result plumbing that does not change what is being checked.
const PLUMBING = new Set(['and_then', 'map', 'cloned', 'copied', 'ok_or', 'ok_or_else', 'or_else', 'or', 'context', 'with_context', 'parse', 'unwrap_or', 'unwrap_or_default', 'unwrap_or_else', 'as_str', 'as_array', 'as_object', 'as_i64', 'as_u64',
  'as_f64', 'as_bool', 'to_owned', 'to_string', 'ok', 'map_err', 'as_ref', 'as_deref', 'clone', 'into']);

/// The index of the `(` matching the `)` that ends `t`, or -1.
function openOf(t) {
  let depth = 0, quote = false;
  for (let i = t.length - 1; i >= 0; i--) {
    const ch = t[i];
    if (ch === '"' && t[i - 1] !== '\\') quote = !quote;
    if (quote) continue;
    if (ch === ')') depth++;
    if (ch === '(' && --depth === 0) return i;
  }
  return -1;
}

/// Code quoted in its short form, the whole of it on hover; a short form
/// cut with "…" opens on click.
function quoted(t, title) {
  const short = shortSubject(t);
  return '<code title="' + esc(title === undefined ? t : title) + '">' + (short.includes('…') ? cutLink(short, (t || '').trim()) : esc(short)) + '</code>';
}

/// `order.get("id").and_then(|v| v.as_str())` → `order.get("id")`;
/// `orders::fetch(settings::get(..), ..)` → `orders::fetch(…)`.
function shortSubject(t) {
  t = (t || '').trim();
  for (let guard = 0; guard < 8 && t.endsWith(')'); guard++) {
    const open = openOf(t);
    if (open < 0) break;
    const m = /\.([A-Za-z_][A-Za-z0-9_]*)(::<[^>]*>)?$/.exec(t.slice(0, open));
    if (!m || !PLUMBING.has(m[1])) break;
    t = t.slice(0, open - m[0].length);
  }
  if (t.endsWith(')')) {
    const open = openOf(t);
    if (open > 0 && t.length - open - 2 > 28) t = t.slice(0, open) + '(…)';
  }
  return t.length > 70 ? t.slice(0, 69) + '…' : t;
}

/// One pattern, said about its subject: `Err(e)` → "… fails".
function patternPhrase(pat, subj, negate) {
  const p = pat.trim();
  const alts = splitTop(p, '|');
  if (alts.length > 1) return alts.map((a) => patternPhrase(a, subj, negate)).join(negate ? ' and ' : ' or ');
  const is = (re) => re.test(p);
  if (p === '_') return subj.html + (subj.call ? ' returns anything else' : ' is anything else');
  if (is(/^Ok\s*\(/)) return subj.html + (negate ? ' fails' : ' succeeds');
  if (is(/^Err\s*\(/)) return subj.html + (negate ? ' succeeds' : ' fails');
  if (is(/^Some\s*\(/)) return subj.html + (negate ? (subj.call ? ' returns nothing' : ' is missing') : (subj.call ? ' returns a value' : ' has a value'));
  if (p === 'None') return subj.html + (negate ? (subj.call ? ' returns a value' : ' has a value') : (subj.call ? ' returns nothing' : ' is missing'));
  if (p === 'true' || p === 'false') return subj.html + ' is ' + (negate ? (p === 'true' ? 'false' : 'true') : p);
  if (is(/^(".*"|-?\d[\d_.]*|'.')$/)) return subj.html + (negate ? ' is not ' : ' is ') + code(p);
  return subj.html + (negate ? ' does not match ' : ' matches ') + code(p);
}

/// Split on a separator that is not inside brackets or quotes.
function splitTop(t, sep) {
  const out = [];
  let depth = 0, quote = false, cur = '';
  for (const ch of t) {
    if (ch === '"') quote = !quote;
    if (!quote && '([{'.includes(ch)) depth++;
    if (!quote && ')]}'.includes(ch)) depth--;
    if (!quote && depth === 0 && ch === sep) { out.push(cur); cur = ''; continue; }
    cur += ch;
  }
  out.push(cur);
  return out.map((x) => x.trim()).filter(Boolean);
}

/// A plain `if` condition: a few everyday checks in words, the rest as code.
/// `turned` says it the other way round, for the side where it does not hold.
function conditionPhrase(h, turned) {
  if (turned) {
    if (h.startsWith('!') && /^![A-Za-z_][\w.]*(\(\))?$/.test(h)) return conditionPhrase(h.slice(1));
    if (/\.(is_empty|is_none|is_some|is_err|is_ok)\(\)$/.test(h)) return conditionPhrase(h.startsWith('!') ? h.slice(1) : '!' + h);
    return code(h) + ' <span class="kw">does not hold</span>';
  }
  const not = h.startsWith('!');
  const body = not ? h.slice(1) : h;
  const m = /^(.+)\.(is_empty|is_none|is_some|is_err|is_ok)\(\)$/.exec(body);
  if (!m) return code(h);
  const said = { is_empty: ['is empty', 'is not empty'], is_none: ['is missing', 'has a value'], is_some: ['has a value', 'is missing'],
    is_err: ['failed', 'succeeded'], is_ok: ['succeeded', 'failed'] }[m[2]];
  return code(m[1]) + ' ' + said[not ? 1 : 0];
}

function blockText(s, prevCall) {
  const h = s.h;
  const lead = (w) => '<span class="kw">' + w + '</span> ';
  if (s.b === 'when' || ((s.b === 'if' || s.b === 'elseif' || s.b === 'letelse') && s.on)) {
    const subj = subjectOf(s, prevCall);
    // A guard narrows an arm: `Some(v) if v > 0`.
    const g = / if (.+)$/.exec(h);
    const pat = g ? h.slice(0, g.index) : h;
    const guard = g ? ' <span class="kw">and</span> ' + code(g[1]) : '';
    const w = s.b === 'elseif' ? 'Otherwise, if' : 'If';
    // An `else if let` is still the alternative to the steps before it.
    return lead(w) + patternPhrase(pat, subj, s.b === 'letelse') + guard;
  }
  switch (s.b) {
    case 'if': return lead('If') + conditionPhrase(h);
    case 'unless': return lead('If') + conditionPhrase(h, true);
    case 'elseif': return '<span class="kw" title="' + esc(ref.chain.length ? 'Checked when step ' + ref.chain.join(', ') + ' does not hold' : '') + '">Otherwise, if</span> ' + conditionPhrase(h);
    case 'else': {
      const c = ref.chain;
      const when = c.length === 1 ? 'when step ' + stepRef(c[0]) + ' does not hold'
        : c.length ? 'when none of steps ' + c.map(stepRef).join(', ') + ' hold' : '';
      return lead('Otherwise').trim() + (when ? ' <span class="kw">(' + when + ')</span>' : '');
    }
    case 'for': {
      const m = /^(.+?) in (.+)$/.exec(h);
      return lead('For each') + (m ? code(m[1]) + ' <span class="kw">in</span> ' + code(m[2]) : code(h));
    }
    case 'while': return lead('While') + conditionPhrase(h);
    case 'loop': return lead('Repeat').trim() + ' <span class="kw">until a step below leaves the loop</span>';
  }
  return code(h);
}

/// A value a later decision reads, being set.
function setText(s) {
  const v = code(s.v), val = code(s.val);
  switch (s.op) {
    case 'let': return 'Start with ' + v + ' = ' + val;
    case '=': return 'Set ' + v + ' to ' + val;
    case '-=': return 'Lower ' + v + ' by ' + val;
    case '+=': return 'Raise ' + v + ' by ' + val;
  }
  return 'Update ' + v + ' ' + code(s.op + ' ' + s.val);
}

function stepText(s, prevCall) {
  if (s.t === 'set') return setText(s);
  if (s.t === 'call') {
    const f = F[s.k];
    let h = f ? labelHTML(f) : esc(s.k);
    // Named, not called: whatever it is given to runs it.
    if (s.h != null) return 'Hand ' + h + (s.h ? ' to ' + code(s.h) : ' on') + ' <span class="q">· not run here</span>';
    if (s.args.length) {
      const a = s.args.join(', ');
      h += ' <span class="args mono">(' + cut(a, 40) + ')</span>';
    }
    if (s.f) h += ' <span class="q">· stops if it fails</span>';
    if (s.x) h += ' <span class="q">· crashes if it fails</span>';
    return h;
  }
  if (s.t === 'exit') return exitText(s);
  return blockText(s, prevCall);
}

const ICON = { call: '→', block: '◇', for: '↻', while: '↻', loop: '↻', fail: '!', crash: '✕', todo: '✕', exit: '■', return: '■', break: '■', continue: '↷' };
const iconOf = (s) => s.t === 'set' ? '≔' : s.t === 'call' ? (s.h != null ? '↦' : ICON.call) : s.t === 'block' ? (ICON[s.b] || ICON.block) : (ICON[s.e] || '■');

/// What each sign means: shown in the key under the steps, and on hover.
const SIGNS = [
  ['→', 'call', 'Runs another of your functions. Click to open it.'],
  ['↦', 'call', 'Hands one of your functions to other code, which runs it — a route handler, a callback, a task to start. It does not run at this step. Click to open it.'],
  ['◇', 'decision', 'A decision: the steps inside run only when the condition holds. Click to fold.'],
  ['↻', 'loop', 'A loop: the steps inside can run many times. Click to fold.'],
  ['≔', 'set', 'Sets a value that a decision further on reads — a flag or a counter.'],
  ['!', 'error', 'Stops with an error, handing it back to whatever called this.'],
  ['✕', 'crash', 'Crashes the program, or reaches code that is not built yet.'],
  ['■', 'stop', 'Stops early without an error, or leaves the loop.'],
  ['↷', 'skip', 'Skips the rest of this round of the loop and starts the next.'],
];
const SIGN_TEXT = Object.fromEntries(SIGNS.map(([sign, , text]) => [sign, text]));
const signTitle = (s) => SIGN_TEXT[iconOf(s)] || '';

const BADGES = [
  ['add', 'new', 'Added by the change.'],
  ['del', 'removed', 'Removed by the change.'],
  ['chg', 'changed', 'Different from before; “was:” shows how it read.'],
  ['inside', 'leads to a change', 'Unchanged itself, but something it runs changed.'],
];
const BADGE_TEXT = Object.fromEntries(BADGES.map(([cls, , text]) => [cls, text]));
const badge = (cls, label) => '<span class="badge ' + cls + '" title="' + esc(BADGE_TEXT[cls]) + '">' + label + '</span>';

/// The key, opened from the header: every sign and badge on the steps, in a
/// line each.
function signKey() {
  const row = (mark, text) => '<li><span class="key-mark">' + mark + '</span><span>' + text + '</span></li>';
  return '<div class="key"><ul>' +
    SIGNS.map(([sign, cls, text]) => row('<span class="key-ic ' + cls + '">' + sign + '</span>', esc(text))).join('') +
    row('<span class="key-ic">A → B</span>', 'On one line: when A holds, B happens.') +
    row('<span class="key-ic gate">◆</span>', 'Main path: the path carries on only when this holds; the other cases hang off it as exits.') +
    row('<span class="key-exit">! if … → stop</span>', 'Main path: a branch that only stops, off to the side of the path. “+N steps” opens a longer one.') +
    row('<span class="key-ic">Only if</span>', 'Main path: steps that run only sometimes, in a lane of their own.') +
    row('<span class="key-ic">Depending on</span>', 'Main path: the path splits; each case that does real work gets its own lane.') +
    row('<span class="key-no">2.3</span>', 'Step 3 inside step 2. Folded steps keep their numbers.') +
    row('<span class="looptag">L1.2</span>', 'A loop’s name: the second loop inside loop L1. “Leave loop L1.2” says which loop a step leaves. Hover a name to see the loop.') +
    row('<span class="key-fold err">! 3 error stops</span>', 'Error handling folded away so the main path reads first. Click to show it.') +
    row('<span class="key-fold">⋯ steps 4–6</span>', 'Steps that did not change, folded away. Click to show them.') +
    BADGES.map(([cls, label, text]) => row('<span class="badge ' + cls + '">' + label + '</span>', esc(text))).join('') +
    row('<code>code</code>', 'Quoted from the source where there is no plain way to say it. Hover for the full text.') +
    '</ul></div>';
}

function toggleSigns(open) {
  const panel = $('signpanel'), btn = $('signsbtn');
  if (open === undefined) open = panel.hidden;
  if (open) {
    togglePatternHelper(false);
    panel.innerHTML = '<div class="rx-head"><b>What the signs mean</b><button class="rx-x" id="signclose" title="Close (Esc)">✕</button></div>' + signKey();
    $('signclose').onclick = () => toggleSigns(false);
  }
  panel.hidden = !open;
  btn.classList.toggle('on', open);
  btn.setAttribute('aria-expanded', String(open));
}

// ------------------------------------------------------------ flowchart

const differs = (r) => r.m !== 'same' || r.c.some(differs);
const leads = (r) => differs(r) || r.c.some(leads) || ((r.a || r.b).t === 'call' && F[(r.a || r.b).k] && affected((r.a || r.b).k));

/// A branch whose only content is one stop: said on one line, "If that
/// fails → stop and pass the error on".
///
/// The same holds for a branch that sets a value or two and then stops:
/// "If it is missing → set all_valid to false, leave the loop".
const outcomeOnly = (r) => !r.c.length && ['exit', 'set'].includes((r.a || r.b).t);
const singleOutcome = (r) => (r.a || r.b).t === 'block' && !['for', 'while', 'loop'].includes((r.a || r.b).b) &&
  r.c.length >= 1 && r.c.length <= 3 && r.c.every(outcomeOnly) &&
  (r.c.length === 1 ? (r.c[0].a || r.c[0].b).t === 'exit' : (r.c[r.c.length - 1].a || r.c[r.c.length - 1].b).t === 'exit');

/// Where each tracked value is set, by the step number the reader sees.
function setSites(rows, prefix, map) {
  rows.forEach((r, i) => {
    const no = prefix + (i + 1), s = r.a || r.b;
    if (s.t === 'set') (map[s.v] = map[s.v] || []).push(no);
    if (singleOutcome(r)) {
      for (const c of r.c) { const cs = c.a || c.b; if (cs.t === 'set') (map[cs.v] = map[cs.v] || []).push(no); }
    } else {
      setSites(r.c, no + '.', map);
    }
  });
  return map;
}

/// "· set in steps 1, 4.1.1, 4.1.2 +3": where the values a decision reads
/// come from.
function setNote(s, sets) {
  if (s.t !== 'block' || s.on || !sets) return '';
  const cond = s.h || '';
  const parts = [];
  for (const v in sets) {
    if (!new RegExp('(^|[^A-Za-z0-9_])' + v + '($|[^A-Za-z0-9_])').test(cond)) continue;
    const at = sets[v];
    parts.push(code(v) + ' set in ' + (at.length === 1 ? 'step ' : 'steps ') + at.slice(0, 4).join(', ') + (at.length > 4 ? ' +' + (at.length - 4) : ''));
  }
  return parts.length ? '<span class="setnote" title="Where the values this decision reads are set">· ' + parts.join('; ') + '</span>' : '';
}

/// Error handling only: stops with an error or a crash, or branches holding
/// nothing else. Never anything that changed.
const errorOnly = (r) => {
  if (differs(r)) return false;
  const s = r.a || r.b;
  if (s.t === 'exit') return ['fail', 'crash', 'todo'].includes(s.e);
  return s.t === 'block' && r.c.length > 0 && r.c.every(errorOnly);
};
const errorStops = (rows) => rows.reduce((n, r) => {
  const s = r.a || r.b;
  return n + (s.t === 'exit' && ['fail', 'crash', 'todo'].includes(s.e) ? 1 : 0) + errorStops(r.c);
}, 0);

/// Whether error handling is shown; `null` until the reader chooses, and
/// until then decided by how long the function is.
let showErrors = null;
const LONG_FUNCTION = 12;

/// Long lists in the details — the glossary, a function's variables — show
/// this many at a time.
const LIST_PAGE = 10;

/// Show the items `selector` finds in `container` a page at a time, with a
/// pager after them: "‹ 1–10 of 16 ›" and a button per page, amber where
/// the page holds a change (`data-changed`). Items marked `data-off` are
/// filtered out and skipped. Returns a function that starts over at page one
/// — for when a filter changes.
function paginate(container, selector) {
  if (!container) return () => {};
  const doc = container.ownerDocument;
  let page = 0;
  const bar = doc.createElement('div');
  bar.className = 'pgbar';
  container.after(bar);
  const draw = () => {
    const all = [...container.querySelectorAll(selector)];
    const on = all.filter((el) => !el.dataset.off);
    const pages = Math.max(1, Math.ceil(on.length / LIST_PAGE));
    page = Math.min(Math.max(page, 0), pages - 1);
    all.forEach((el) => (el.hidden = true));
    on.slice(page * LIST_PAGE, (page + 1) * LIST_PAGE).forEach((el) => (el.hidden = false));
    bar.hidden = on.length <= LIST_PAGE;
    if (bar.hidden) return;
    const from = page * LIST_PAGE + 1, to = Math.min(on.length, (page + 1) * LIST_PAGE);
    let h = '<button class="pgb" data-p="' + (page - 1) + '"' + (page ? '' : ' disabled') + ' aria-label="Previous page">‹</button>' +
      '<span class="pg-range">' + from + '–' + to + ' of ' + on.length + '</span>';
    for (let p = 0; p < pages; p++) {
      const changed = on.slice(p * LIST_PAGE, (p + 1) * LIST_PAGE).some((el) => el.dataset.changed);
      h += '<button class="pgb num' + (p === page ? ' on' : '') + (changed ? ' marked' : '') + '" data-p="' + p + '" title="' +
        (p * LIST_PAGE + 1) + '–' + Math.min(on.length, (p + 1) * LIST_PAGE) + (changed ? ' — something here changed' : '') + '">' + (p + 1) + '</button>';
    }
    h += '<button class="pgb" data-p="' + (page + 1) + '"' + (page < pages - 1 ? '' : ' disabled') + ' aria-label="Next page">›</button>';
    bar.innerHTML = h;
    bar.querySelectorAll('.pgb').forEach((b) => (b.onclick = (ev) => { ev.stopPropagation(); page = +b.dataset.p; draw(); }));
  };
  draw();
  // Turn to the page holding `el`, for links that point into the list.
  container.showItem = (el) => {
    const on = [...container.querySelectorAll(selector)].filter((x) => !x.dataset.off);
    const i = on.indexOf(el);
    if (i >= 0) { page = Math.floor(i / LIST_PAGE); draw(); }
  };
  return () => { page = 0; draw(); };
}

/// A fold button that opens the steps it stands for just below itself, and
/// folds them away again: "· show" ↔ "· hide". The steps are drawn once.
function foldToggle(b, render) {
  let inner = null;
  b.setAttribute('aria-expanded', 'false');
  b.onclick = () => {
    if (!inner) {
      inner = document.createElement('div');
      inner.className = 'folded';
      render(inner);
      b.after(inner);
    } else {
      inner.hidden = !inner.hidden;
    }
    const open = !inner.hidden;
    b.classList.toggle('open', open);
    b.setAttribute('aria-expanded', String(open));
    const act = b.querySelector('.fold-act');
    if (act) act.textContent = open ? '· hide' : '· show';
  };
}

/// Fold and unfold a lane of the main path from its header line: ▾ open,
/// ▸ folded. `bodies` are what folds — one lane, or a fork's every lane.
function collapsible(head, bodies) {
  head.classList.add('folds');
  head.insertAdjacentHTML('beforeend', '<span class="caret" aria-hidden="true">▾</span>');
  head.title = 'Fold the steps inside';
  head.onclick = () => {
    const open = bodies.some((b) => b.hidden);
    bodies.forEach((b) => (b.hidden = !open));
    head.querySelector('.caret').textContent = open ? '▾' : '▸';
    head.title = open ? 'Fold the steps inside' : 'Show the steps inside';
  };
}

/// One function's steps as a vertical flowchart, changes coloured in place.
///
/// Steps are numbered like an outline — 1, 2, 3, then 3.1, 3.2 inside a
/// branch. A branch or loop holds its steps in a container that folds with a
/// click; a branch with a single stop in it is one line. Runs of unchanged
/// steps fold away when the function changed, and runs of error handling fold
/// away when the reader hides it; folded steps keep their numbers.
/// `offset` numbers a folded run from where it sits in the full list.
function flowchart(box, rows, prefix, opts, offset = 0) {
  let i = 0;
  let prevCall = null, prevNo = null;
  let chain = [];
  const loops = opts.loops || [];
  while (i < rows.length) {
    // Error handling, hidden: one line per run, opened on a click.
    if (!opts.errors && errorOnly(rows[i])) {
      let j = i;
      while (j < rows.length && errorOnly(rows[j])) j++;
      const run = rows.slice(i, j), from = offset + i;
      const n = errorStops(run);
      const b = document.createElement('button');
      b.className = 'fold err';
      b.title = 'Error handling, folded so the main path reads first. Click to show it.';
      b.innerHTML = '<b>!</b> ' + plural(n, 'error stop', 'error stops') + ' <span class="no-inline">' +
        prefix + (from + 1) + (run.length > 1 ? '–' + prefix + (from + run.length) : '') + '</span> <span class="fold-act">· show</span>';
      foldToggle(b, (inner) => flowchart(inner, run, prefix, { ...opts, errors: true, fold: false }, from));
      box.appendChild(b);
      i = j;
      prevCall = null;
      continue;
    }
    if (opts.fold) {
      let j = i;
      while (j < rows.length && !leads(rows[j])) j++;
      if (j - i >= 2) {
        const run = rows.slice(i, j), from = offset + i;
        const b = document.createElement('button');
        b.className = 'fold';
        b.title = 'These steps are the same as before. Click to show them.';
        b.innerHTML = '⋯ steps ' + prefix + (from + 1) + '–' + prefix + (from + run.length) + ' did not change <span class="fold-act">· show</span>';
        foldToggle(b, (inner) => flowchart(inner, run, prefix, { ...opts, fold: false }, from));
        box.appendChild(b);
        i = j;
        prevCall = null;
        continue;
      }
    }
    const r = rows[i];
    const no = prefix + (offset + i + 1);
    i++;
    const s = r.a || r.b;
    // An if-chain: `if` starts it, `else if` and `else` continue it.
    if (!(s.t === 'block' && (s.b === 'elseif' || s.b === 'else'))) chain = [];
    ref.loops = loops;
    ref.that = prevNo;
    ref.chain = chain.slice();
    const merged = singleOutcome(r);
    const outs = merged ? r.c : [];
    const last = merged ? outs[outs.length - 1] : null;
    const changedOut = outs.find((c) => c.m !== 'same');
    const mark = r.m !== 'same' ? r.m : changedOut ? changedOut.m : r.m;
    const look = merged ? last.a || last.b : s;
    const outcomes = (side) => outs.filter((c) => c[side]).map((c) => lower(stepText(c[side]))).join(', ');

    const el = document.createElement('div');
    el.className = 'st ' + look.t + ' ' + (look.t === 'exit' ? look.e + ' ' : '') + mark + (merged ? ' merged' : '');
    let text = stepText(s, prevCall);
    if (merged) text += ' <span class="then" title="When the condition holds, this happens">→</span> ' + outcomes(r.a ? 'a' : 'b');
    // Only on a decision that reads the value; a line that sets it says enough.
    if (!merged) text += setNote(s, opts.sets);
    // A branch kept only so the ones after it can say what they are the
    // alternative to.
    if (s.t === 'block' && !r.c.length) text += ' <span class="kw">— nothing of note happens</span>';
    const where = no.includes('.') ? 'Step ' + no + ' — inside step ' + no.slice(0, no.lastIndexOf('.')) : 'Step ' + no;
    const loopId = opts.loopNames && opts.loopNames.get(r);
    if (loopId) text = '<span class="looptag own" title="This loop is called ' + loopId + '">' + loopId + '</span> ' + text;
    let h = '<span class="no"' + lineOf(r) + ' title="' + where + NO_TITLE + '">' + no + '</span><span class="ic" title="' + esc(signTitle(look)) + '">' + iconOf(look) +
      '</span><span class="body"><span class="t">' + text + '</span>';
    if (mark === 'add') h += badge('add', 'new');
    if (mark === 'del') h += badge('del', 'removed');
    if (mark === 'chg') h += badge('chg', 'changed');
    if (s.t === 'call' && F[s.k] && r.m === 'same') {
      const c = F[s.k];
      if (c.s === 'changed') h += badge('chg', 'changed');
      else if (c.s === 'added') h += badge('add', 'new');
      else if (c.cb.length) h += badge('inside', 'leads to a change');
    }
    if (mark === 'chg') {
      const was = merged
        ? (r.b ? stepText(r.b, prevCall) : stepText(s, prevCall)) + ' → ' + outcomes('b')
        : r.b ? stepText(r.b, prevCall) : '';
      if (was) h += '<span class="was">was: ' + was + '</span>';
    }
    h += '</span>';
    el.innerHTML = h;
    box.appendChild(el);
    // Arms of one match all check the call before them: keep naming it.
    if (s.t === 'call') { prevCall = s.k; prevNo = no; }
    else if (!(s.t === 'block' && s.oc && s.oc === prevCall)) { prevCall = null; prevNo = null; }
    if (s.t === 'block' && (s.b === 'if' || s.b === 'elseif')) chain.push(no);

    if (s.t === 'call' && F[s.k]) {
      el.title = 'Open ' + F[s.k].l;
      el.onclick = () => select(s.k, true);
      // Point at the box this step calls.
      const boxes = () => document.querySelectorAll('.node[data-k="' + CSS.escape(s.k) + '"]');
      el.onmouseenter = () => boxes().forEach((n) => n.classList.add('hover'));
      el.onmouseleave = () => boxes().forEach((n) => n.classList.remove('hover'));
    }
    if (r.c.length && !merged) {
      const kids = document.createElement('div');
      kids.className = 'kids';
      box.appendChild(kids);
      // A loop's steps know they are inside it, so "leave the loop" can name it.
      const isLoop = s.t === 'block' && ['for', 'while', 'loop'].includes(s.b);
      const inner = isLoop ? loops.concat([loopInfo(s, no, opts.loopNames && opts.loopNames.get(r))]) : loops;
      flowchart(kids, r.c, no + '.', { ...opts, loops: inner });
      el.classList.add('open');
      el.insertAdjacentHTML('beforeend', '<span class="caret" aria-hidden="true">▾</span>');
      el.title = 'Fold the steps inside';
      el.onclick = () => {
        kids.hidden = !kids.hidden;
        el.classList.toggle('open', !kids.hidden);
        el.querySelector('.caret').textContent = kids.hidden ? '▸' : '▾';
        el.title = kids.hidden ? 'Show the ' + plural(countSteps(r.c), 'step', 'steps') + ' inside' : 'Fold the steps inside';
      };
    }
  }
}

/// How a loop is referred to when a step leaves it or skips ahead in it.
function loopInfo(s, no, id) {
  const m = s.b === 'for' ? /^(.+?) in (.+)$/.exec(s.h) : null;
  const what = s.b === 'for' ? 'For each ' + (m ? m[1] + ' in ' + m[2] : s.h) : s.b === 'while' ? 'While ' + s.h : 'Repeat';
  return { no, id: id || 'L?', title: 'Loop ' + (id || '') + ' — ' + what + ' (step ' + no + ')', each: s.b === 'for' ? code(m ? m[1] : s.h) : null };
}

const lower = (html) => html.replace(/^(<[^>]+>)?([A-Z])/, (m, tag, c) => (tag || '') + c.toLowerCase());

// ------------------------------------------------------------ main path
//
// The same steps, read for the path the code normally takes. Calls, values
// set and loops run down one line. A branch that only stops — "value is
// missing → leave loop L1" — hangs off that line as a side exit. When a
// decision has one side that carries on and others that only stop, the line
// passes through it as a gate; when several sides do real work it forks into
// lanes; a branch with work and no alternative is an optional lane, never
// drawn as if it always ran.

/// Ends in a stop: its last step leaves the function, the loop or the round.
const stops = (rows) => rows.length > 0 && (rows[rows.length - 1].a || rows[rows.length - 1].b).t === 'exit';

/// Steps grouped into decisions: a match's arms, an if-chain's branches, or a
/// single turned-round `if`. `full` says every case is covered — a match, or
/// an if-chain ending in `else` — so a gate can say the rest only stops.
function groupSteps(rows) {
  const out = [];
  const st = (r) => r.a || r.b;
  for (let i = 0; i < rows.length; i++) {
    const r = rows[i], s = st(r);
    if (s.t !== 'block' || ['for', 'while', 'loop'].includes(s.b)) { out.push({ step: r, i }); continue; }
    const arms = [{ r, i }];
    if (s.b === 'when') {
      while (i + 1 < rows.length) {
        const n = st(rows[i + 1]);
        if (n.t === 'block' && n.b === 'when' && n.on === s.on && n.oc === s.oc) arms.push({ r: rows[++i], i });
        else break;
      }
      out.push({ arms, full: true });
    } else if (s.b === 'if') {
      while (i + 1 < rows.length) {
        const n = st(rows[i + 1]);
        if (n.t === 'block' && (n.b === 'elseif' || n.b === 'else')) arms.push({ r: rows[++i], i });
        else break;
      }
      out.push({ arms, full: arms.some((a) => st(a.r).b === 'else') });
    } else {
      // `unless`, `let … else`, or a stray branch: one side, the other carries on.
      out.push({ arms, full: s.b === 'unless' || s.b === 'letelse' });
    }
  }
  return out;
}

/// The change badges for a step, the same as in the full list.
function badgesOf(r, s, mark) {
  let h = '';
  if (mark === 'add') h += badge('add', 'new');
  if (mark === 'del') h += badge('del', 'removed');
  if (mark === 'chg') h += badge('chg', 'changed');
  if (s.t === 'call' && F[s.k] && mark === 'same') {
    const c = F[s.k];
    if (c.s === 'changed') h += badge('chg', 'changed');
    else if (c.s === 'added') h += badge('add', 'new');
    else if (c.cb.length) h += badge('inside', 'leads to a change');
  }
  return h;
}

/// One step on the line.
function spineStep(r, no, text, variant, icon) {
  const s = r.a || r.b;
  const mark = r.m !== 'same' ? r.m : r.c.some(differs) && variant !== 'loop' ? r.m : r.m;
  const el = document.createElement('div');
  el.className = 'mp-step ' + variant + ' ' + mark + (s.t === 'exit' ? ' ' + s.e : '');
  el.innerHTML = '<span class="mp-ic" title="' + esc(signTitle(s)) + '">' + (icon || iconOf(s)) + '</span>' +
    '<span class="body"><span class="t">' + text + '</span>' + badgesOf(r, s, mark) +
    (mark === 'chg' && r.b ? '<span class="was">was: ' + stepText(r.b) + '</span>' : '') + '</span>' +
    '<span class="mp-no"' + lineOf(r) + ' title="Step ' + no + NO_TITLE + '">' + no + '</span>';
  if (s.t === 'call' && F[s.k]) {
    el.title = 'Open ' + F[s.k].l;
    el.classList.add('opens');
    el.onclick = () => select(s.k, true);
    const boxes = () => document.querySelectorAll('.node[data-k="' + CSS.escape(s.k) + '"]');
    el.onmouseenter = () => boxes().forEach((n) => n.classList.add('hover'));
    el.onmouseleave = () => boxes().forEach((n) => n.classList.remove('hover'));
  }
  return el;
}

/// A branch that only stops, on one line off the main path: "if X → stop".
/// A longer one shows how it ends, and opens to show the steps before that.
function sideExit(a, no, opts, prevCall) {
  const s = a.a || a.b;
  const last = a.c[a.c.length - 1], ls = last.a || last.b;
  const kind = ['fail', 'crash', 'todo'].includes(ls.e) ? 'err' : ['break', 'continue'].includes(ls.e) ? 'loopx' : 'stop';
  const changed = differs(a);
  const el = document.createElement('div');
  el.className = 'mp-exit ' + kind + (changed ? ' chg' : '');
  const cond = stepText(s, prevCall);
  let text;
  if (singleOutcome(a)) {
    text = cond + ' <span class="then">→</span> ' + a.c.map((c) => lower(stepText(c.a || c.b))).join(', ');
  } else {
    const before = countSteps(a.c) - 1;
    text = cond + ' <span class="then">→</span> … ' + lower(stepText(ls)) +
      ' <button class="mp-more" title="Show the steps in this branch">+' + plural(before, 'step', 'steps') + '</button>';
  }
  el.innerHTML = '<span class="mp-x" title="' + esc(signTitle(ls)) + '">' + iconOf(ls) + '</span><span class="body">' + text +
    (changed ? badge('chg', 'changed') : '') + '</span><span class="mp-no"' + lineOf(a) + ' title="Step ' + no + NO_TITLE + '">' + no + '</span>';
  const more = el.querySelector('.mp-more');
  if (more) {
    let box = null;
    more.onclick = (ev) => {
      ev.stopPropagation();
      if (!box) {
        box = document.createElement('div');
        box.className = 'mp-detail';
        flowchart(box, a.c, no + '.', { ...opts, errors: true, fold: false });
        el.after(box);
      } else {
        box.hidden = !box.hidden;
      }
      more.textContent = box.hidden ? '+' + plural(countSteps(a.c) - 1, 'step', 'steps') : 'hide';
    };
  }
  return el;
}

/// A stop written straight into the steps — a `?` or an `.unwrap()` — as an
/// exit off the path.
function exitLine(r, no) {
  const s = r.a || r.b;
  const changed = differs(r);
  const el = document.createElement('div');
  el.className = 'mp-exit err' + (changed ? ' ' + r.m : '');
  el.innerHTML = '<span class="mp-x" title="' + esc(signTitle(s)) + '">' + iconOf(s) + '</span><span class="body">' + stepText(s) +
    badgesOf(r, s, r.m) + (r.m === 'chg' && r.b ? '<span class="was">was: ' + stepText(r.b) + '</span>' : '') +
    '</span><span class="mp-no"' + lineOf(r) + ' title="Step ' + no + NO_TITLE + '">' + no + '</span>';
  return el;
}

/// Render `rows` as a main path into `box`.
function mainPath(box, rows, prefix, opts) {
  const loops = opts.loops || [];
  let prevCall = null, prevNo = null;
  const lane = (cls) => { const d = document.createElement('div'); d.className = 'mp-lane ' + cls; box.appendChild(d); return d; };

  for (const g of groupSteps(rows)) {
    ref.loops = loops;
    ref.that = prevNo;
    ref.chain = [];
    if (g.step) {
      const r = g.step, s = r.a || r.b, no = prefix + (g.i + 1);
      if (s.t === 'block') {
        // A loop stays on the line; its body runs down a lane of its own.
        const id = opts.loopNames && opts.loopNames.get(r);
        const tag = id ? '<span class="looptag own" title="This loop is called ' + id + '">' + id + '</span> ' : '';
        const head = spineStep(r, no, tag + stepText(s), 'loop');
        box.appendChild(head);
        const body = lane('loop');
        mainPath(body, r.c, no + '.', { ...opts, loops: loops.concat([loopInfo(s, no, id)]) });
        collapsible(head, [body]);
      } else if (s.t === 'exit' && ['fail', 'crash', 'todo'].includes(s.e)) {
        // `?`, `.unwrap()`, `panic!`: the path carries on past them, so they
        // sit off to the side like any other way out.
        box.appendChild(exitLine(r, no));
      } else {
        box.appendChild(spineStep(r, no, stepText(s, prevCall), s.t === 'exit' ? 'stopper' : 'plain'));
      }
      if (s.t === 'call') { prevCall = s.k; prevNo = no; } else { prevCall = null; prevNo = null; }
      continue;
    }

    // A decision.
    const arms = g.arms.map((a) => ({ ...a, no: prefix + (a.i + 1), s: a.r.a || a.r.b }));
    const exits = arms.filter((a) => stops(a.r.c));
    const work = arms.filter((a) => !stops(a.r.c));
    // "Otherwise" arms name what they are the alternative to.
    const chainBefore = (a) => arms.filter((x) => x.i < a.i && (x.s.b === 'if' || x.s.b === 'elseif')).map((x) => x.no);
    // Lanes drawn before an exit leave their own loops in `ref`: each exit
    // and arm restores this level's, so "leave loop" names the right one.
    const exitOf = (a) => { ref.loops = loops; ref.chain = chainBefore(a); ref.that = prevNo; return sideExit(a.r, a.no, opts, prevCall); };

    if (!work.length) {
      for (const a of exits) box.appendChild(exitOf(a));
    } else if (work.length === 1 && g.full && exits.length) {
      // A gate: one side carries on, the rest only stop.
      const w = work[0];
      ref.chain = chainBefore(w);
      const text = stepText(w.s, prevCall) + setNote(w.s, opts.sets);
      box.appendChild(spineStep(w.r, w.no, text, 'gate', '◆'));
      for (const a of exits) box.appendChild(exitOf(a));
      mainPath(box, w.r.c, w.no + '.', opts);
    } else if (work.length === 1 && !exits.length && !g.full) {
      // Work with no alternative: optional, in a lane of its own.
      const w = work[0];
      const text = '<span class="kw">Only</span> ' + lower(stepText(w.s, prevCall)) + setNote(w.s, opts.sets);
      const head = spineStep(w.r, w.no, text, 'optional', '◇');
      box.appendChild(head);
      const body = lane('optional');
      mainPath(body, w.r.c, w.no + '.', opts);
      collapsible(head, [body]);
    } else {
      // Several sides do real work: the line forks.
      const first = arms[0].s;
      const subject = first.on || first.oc ? subjectOf(first, prevCall).html : null;
      const head = document.createElement('div');
      head.className = 'mp-step fork same';
      head.innerHTML = '<span class="mp-ic" title="' + esc(SIGN_TEXT['◇']) + '">◇</span><span class="body"><span class="t">' +
        (subject ? '<span class="kw">Depending on</span> ' + subject : '<span class="kw">Depending on which holds</span>') +
        '</span></span><span class="mp-no"' + lineOf(arms[0].r) + ' title="Steps ' + arms[0].no + NO_TITLE + '">' + arms[0].no + (arms.length > 1 ? '–' + arms[arms.length - 1].no : '') + '</span>';
      box.appendChild(head);
      const lanes = [];
      for (const a of arms) {
        if (stops(a.r.c)) { box.appendChild(exitOf(a)); continue; }
        ref.loops = loops;
        ref.chain = chainBefore(a);
        const l = lane('fork');
        lanes.push(l);
        const armHead = spineStep(a.r, a.no, stepText(a.s, prevCall) + setNote(a.s, opts.sets), 'arm', '◇');
        l.appendChild(armHead);
        const body = document.createElement('div');
        body.className = 'mp-armbody';
        l.appendChild(body);
        mainPath(body, a.r.c, a.no + '.', opts);
        collapsible(armHead, [body]);
      }
      collapsible(head, lanes);
      if (!g.full) {
        const rest = document.createElement('div');
        rest.className = 'mp-exit carry';
        rest.innerHTML = '<span class="mp-x">↓</span><span class="body"><span class="kw">Otherwise: carries on below</span></span>';
        box.appendChild(rest);
      }
    }
    prevCall = null;
    prevNo = null;
    ref.loops = loops;
  }
}

/// Which way the steps are shown: the main path, or every step in order.
let stepView = 'code';

const countSteps = (rows) => rows.reduce((n, r) => n + 1 + countSteps(r.c), 0);

/// Every way a function can finish, counted on one side of the comparison:
/// `a` for now, `b` for before.
function endings(rows, side, acc) {
  acc = acc || { fail: 0, crash: 0, todo: 0, ret: 0, exit: 0 };
  for (const r of rows) {
    const s = r[side];
    if (s) {
      if (s.t === 'exit') {
        const k = { fail: 'fail', crash: 'crash', todo: 'todo', return: 'ret', exit: 'exit' }[s.e];
        if (k) acc[k]++;
      }
      if (s.t === 'call' && s.f) acc.fail++;
      if (s.t === 'call' && s.x) acc.crash++;
    }
    endings(r.c, side, acc);
  }
  return acc;
}

const ENDING = [
  ['fail', '!', 'way to stop with an error', 'ways to stop with an error', 'fail'],
  ['crash', '✕', 'way to crash', 'ways to crash', 'crash'],
  ['todo', '✕', 'unfinished part', 'unfinished parts', 'crash'],
  ['ret', '■', 'early return', 'early returns', ''],
  ['exit', '■', 'place it quits the program', 'places it quits the program', ''],
];

/// "How it can end": the first thing a reviewer asks of a function. For a
/// changed one, each count shows what it was before.
function endingsHTML(f) {
  const now = endings(f.rows, f.s === 'removed' ? 'b' : 'a');
  const before = f.s === 'changed' ? endings(f.rows, 'b') : null;
  const chips = [];
  for (const [k, ic, one, many, cls] of ENDING) {
    const n = now[k], was = before ? before[k] : n;
    if (!n && !was) continue;
    const moved = before && n !== was;
    chips.push('<span class="end ' + cls + (moved ? ' moved' : '') + '" title="' + esc(SIGN_TEXT[ic] + (moved ? ' The count changed: before → now.' : '')) + '"><b>' + ic + '</b> ' +
      (moved ? was + ' → ' + n + ' ' + (n === 1 ? one : many) : plural(n, one, many)) + '</span>');
  }
  if (!chips.length) return '<p class="ends-none">Always runs to the end — no early stops, errors or crashes.</p>';
  return '<div class="ends">' + chips.join('') + '</div>';
}

/// Rust keywords and the everyday variants, coloured in the code view.
const KEYWORDS = /\b(fn|let|mut|if|else|match|for|while|loop|return|break|continue|in|pub|async|await|impl|struct|enum|use|mod|as|ref|move|where|const|static|unsafe|dyn|crate|super|self|Self|Some|None|Ok|Err|true|false)\b/;

/// One line of source, lightly coloured: comments, strings, keywords.
function highlight(line) {
  const re = new RegExp('(//.*$)|("(?:[^"\\\\]|\\\\.)*")|' + KEYWORDS.source, 'g');
  let out = '', last = 0, m;
  while ((m = re.exec(line))) {
    out += esc(line.slice(last, m.index));
    const cls = m[1] ? 'c-com' : m[2] ? 'c-str' : 'c-kw';
    out += '<span class="' + cls + '">' + esc(m[0]) + '</span>';
    last = m.index + m[0].length;
    if (!m[0].length) re.lastIndex++;
  }
  return out + esc(line.slice(last));
}

/// The function's source, before and now: line numbers on both sides, added
/// lines marked +, removed ones −. Each row carries its line numbers, so a
/// step can point at its own line.
function codeTable(f) {
  // Before and now numbers only when the lines differ; otherwise one.
  const two = f.s === 'changed';
  // Indents, measured in spaces (a tab as four): the part every line shares
  // is dropped, and the deepest one sets how far the rest can be scaled down
  // in a narrow panel.
  const leads = f.code.filter(([, , , t]) => /\S/.test(t)).map(([, , , t]) => indentOf(t));
  const base = leads.length ? Math.min(...leads) : 0;
  const deepest = Math.max(1, ...leads.map((n) => n - base));
  return '<div class="code"><table style="--deepest:' + deepest + '">' + f.code.map(([m, b, a, t]) =>
    '<tr class="' + m + '"' + (b ? ' data-b="' + b + '"' : '') + (a ? ' data-a="' + a + '"' : '') + '>' +
    (two ? '<td class="n">' + (b || '') + '</td><td class="n">' + (a || '') + '</td>' : '<td class="n">' + (a || b || '') + '</td>') +
    '<td class="s">' + srcLine(t, base) + '</td></tr>').join('') + '</table></div>';
}

/// How far a line is indented, in spaces; a tab counts as four.
const indentOf = (t) => /^[ \t]*/.exec(t)[0].replace(/\t/g, '    ').length;

/// A source line that may wrap: its indent kept apart, so a wrapped part
/// lines up under the line's own text, a little in, as an editor wraps. The
/// indent's width is set in the stylesheet from `--n`, so a narrow panel
/// can scale every level down alike instead of squeezing the code.
function srcLine(t, base) {
  const [, lead, rest] = /^(\s*)([\s\S]*)$/.exec(t);
  const n = /\S/.test(t) ? Math.max(0, indentOf(t) - base) : 0;
  return '<span class="ind" style="--n:' + n + '">' + lead.replace(/\t/g, '    ').slice(base) + '</span><span class="txt">' + highlight(rest) + '</span>';
}

/// The line a step stands for: on the "now" side when it exists there.
const lineOf = (r) => ' data-ln="' + (r.a || r.b).ln + '" data-side="' + (r.a ? 'a' : 'b') + '"';
const NO_TITLE = ' — click to see its code';

/// Where the code view should open, once it is drawn.
let pendingLine = null;

/// Clickable function names. `how(k)` may say how the link itself changed:
/// a call added or removed is marked on the chip, not just the callee.
function chipList(keys, how) {
  if (!keys.length) return '<span class="empty">nothing</span>';
  return '<div class="chips">' + keys.map((k) => {
    const link = how ? how(k) : null;
    const cls = link === 'add' ? 'added' : link === 'del' ? 'removed' : F[k].s;
    const note = link === 'add' ? ' · new call' : link === 'del' ? ' · no longer called' : '';
    const d = distinguisher(k);
    return '<button class="fchip ' + cls + '" data-k="' + esc(k) + '" title="' + esc(fullName(F[k]) + ' — ' + F[k].f + ':' + F[k].ln) + '">' + labelHTML(F[k]) +
      (d ? ' <span class="ctx mono">' + esc(d) + '</span>' : '') + note + '</button>';
  }).join('') + '</div>';
}

const linkKind = (u, v) => {
  const now = F[u].ce.includes(v), before = F[u].cbe.includes(v);
  return now && before ? 'same' : now ? 'add' : 'del';
};

// ------------------------------------------------------------ panel

function showFunction(k) {
  const f = F[k];
  const panel = P('panel');
  const by = (callers[k] || []).filter(onMap);
  noteView({ t: 'fn', k });
  let h = '<div class="pad">' + navBar() + '<div>' +
    (PLAIN || !DATA.history ? '' : '<span class="pill ' + f.s + '">' + STATUS_WORD[f.s] + '</span> ') +
    (f.ft ? '<span class="pill feature">Feature</span> ' : '') + (f.t ? '<span class="pill same">Test</span>' : '') +
    '</div><h2 class="mono fname">' + esc(fnName(f)) + '</h2><div class="where mono">' + esc(crateOf(f) + f.f) + ':' + f.ln + '</div>' +
    '<div class="where mono modline" title="The module it is defined in">module ' + esc(moduleOf(f)) + (f.o ? ' · impl ' + esc(f.o) : '') + '</div>';
  if (onMap(k) && calls(k).length) {
    h += state.aside.includes(k)
      ? ' <button class="link focusbtn" id="asidebtn" title="Return this flow to the main map">Put back into the main map</button>'
      : ' <button class="link focusbtn" id="asidebtn" title="Move the flow that starts here into its own map beside the main one">⇱ Take out to side space</button>';
  }
  h += ' <button class="link focusbtn" id="codebtn" title="Show this function’s source">‹/› Code</button>';
  if (onMap(k)) {
    h += hidden.has(k)
      ? ' <button class="link focusbtn" id="hidebtn" title="Bring it back onto the map">Show on map</button>'
      : ' <button class="link focusbtn" id="hidebtn" title="Take it off the map, with what is reached only through it (Delete)">✕ Hide from map</button>';
  }
  if (hidden.has(k)) h += '<p class="note">Hidden from the map. Its details stay readable here.</p>';
  if (onMap(k)) {
    h += state.focus === k
      ? '<button class="link focusbtn" id="focusbtn" title="Bring the rest of the map back">Show the whole map</button>'
      : '<button class="link focusbtn" id="focusbtn" title="Hide everything except this, what leads to it, and what it calls">Focus on this</button>';
  }
  if (f.sig) h += '<p class="note">Takes different inputs now: ' + code(f.sig[0]) + ' → ' + code(f.sig[1]) + '</p>';
  if (f.doc) h += '<div class="doc">' + docHTML(f.doc) + '</div>';
  if (f.d) h += '<p class="note">Same steps, but something inside changed — a value, a condition, or a library call. See the code.</p>';
  if (f.s === 'same' && f.cb.length) h += '<p class="note">Unchanged itself, but leads to: ' + f.cb.map((c) => esc(F[c].l)).join(', ') + '</p>';
  if (f.s === 'added') h += '<p class="note" style="background:var(--add);color:var(--add-ink)">New — this did not exist before.</p>';
  if (f.s === 'removed') h += '<p class="note" style="background:var(--del);color:var(--del-ink)">Removed — these are the steps it used to take.</p>';
  // The body: steps as the main column; facts, glossary and connections
  // beside them when the panel is wide, around them when it is narrow.
  h += '<div class="fn-body"><div class="fn-facts facts">';
  h += '<div class="rel"><b>Takes</b>' + (f.p.length ? '<ul class="params">' + f.p.map(([n, t]) =>
    '<li><span class="mono pn">' + esc(n) + '</span> <span class="mono pt">' + linkTypes(t) + '</span></li>').join('') + '</ul>' : '<span class="empty">nothing</span>') + '</div>';
  h += '<div class="rel"><b>Gives back</b>' + (f.r ? '<code>' + linkTypes(f.r) + '</code>' : '<span class="empty">nothing</span>') + '</div>';
  h += '<div class="rel"><b>How it can end</b>' + endingsHTML(f) + '</div>';
  h += '</div><div class="fn-main">';
  const total = countSteps(f.rows), errs = errorStops(f.rows);
  const errorsShown = showErrors === null ? total <= LONG_FUNCTION : showErrors;
  h += '<div class="h3"><span>Steps</span><span class="stepviews">' +
    '<span class="seg mini" role="group" aria-label="How to show the steps">' +
    '<button id="sv-code" class="' + (stepView === 'code' ? 'on' : '') + '" title="The source, with what changed marked">Code</button>' +
    '<button id="sv-all" class="' + (stepView === 'all' ? 'on' : '') + '" title="Every step in code order, numbered">All steps</button>' +
    '<button id="sv-main" class="' + (stepView === 'main' ? 'on' : '') + '" title="The path the code normally takes; branches that only stop hang off it">Main path</button></span>' +
    (errs && stepView === 'all'
      ? '<button class="link" id="errtoggle" title="Error handling: stops with an error or a crash">' + (errorsShown ? 'Hide error handling' : 'Show error handling') + '</button>'
      : '') + '</span></div><div class="fc" id="fc"></div></div>';
  // After the steps: the words they use, then how it connects to the rest.
  h += '<div class="fn-gloss">' + fnGlossaryHTML(f) + '</div>';
  h += '<div class="fn-conn"><div class="h3"><span>Connections</span></div><div class="facts links">' +
    '<div class="rel"><b>Used by</b>' + (f.ft ? '<span class="empty">nothing — it is where a feature starts</span>' : chipList(by, (c) => linkKind(c, k))) + '</div>' +
    '<div class="rel"><b>Uses</b>' + chipList(calls(k), (c) => linkKind(k, c)) + '</div></div></div>';
  h += '</div></div>';
  panel.innerHTML = h;
  const fc = P('fc');
  const stepOpts = { fold: f.s !== 'added' && f.s !== 'removed' && f.rows.some(leads), errors: errorsShown, sets: setSites(f.rows, '', {}), loopNames: loopNames(f.rows) };
  if (stepView === 'code') {
    fc.innerHTML = (f.s === 'changed' ? '<p class="code-note">Lines marked <b>+</b> were added and <b>−</b> removed; the left numbers are before, the right ones now.</p>' : '') + codeTable(f);
    const at = pendingLine;
    pendingLine = null;
    if (at) {
      const row = fc.querySelector('tr[data-' + at.side + '="' + at.ln + '"]');
      if (row) {
        row.classList.add('flash');
        requestAnimationFrame(() => row.scrollIntoView({ block: 'center' }));
      }
    }
  } else if (f.rows.length && stepView === 'main') {
    fc.classList.add('mp');
    mainPath(fc, f.rows, '', stepOpts);
  } else if (f.rows.length) {
    flowchart(fc, f.rows, '', stepOpts);
  }
  // Nothing to put in words — all it does is call library code, like
  // building a router — so show that code rather than an empty space.
  else fc.innerHTML = '<p class="code-note">No steps of its own: everything it does is done by library code, shown here.</p>' + codeTable(f);
  wireNav();
  wireGlossary(panel, k);
  paginate(panel.querySelector('table.vars'), 'tr.vrow');
  linkStepNames(fc, panel, k);
  paginate(panel.querySelector('.gcards'), '.gcard');
  // A line number in the glossary opens the code at that line.
  panel.querySelectorAll('.lnref').forEach((a) => (a.onclick = (ev) => {
    ev.stopPropagation();
    pendingLine = { ln: a.dataset.ln, side: f.s === 'removed' ? 'b' : 'a' };
    stepView = 'code';
    showFunction(k);
  }));
  P('sv-main').onclick = () => { stepView = 'main'; showFunction(k); };
  P('sv-code').onclick = () => { stepView = 'code'; showFunction(k); };
  if (P('codebtn')) P('codebtn').onclick = () => { stepView = 'code'; showFunction(k); };
  // A step's number opens the code at that step's line. Caught on the way
  // down, before the step's own click (open, fold) can act.
  fc.addEventListener('click', (e) => {
    const n = e.target.closest('[data-ln]');
    if (!n) return;
    e.stopPropagation();
    pendingLine = { ln: n.dataset.ln, side: n.dataset.side === 'b' ? 'b' : 'a' };
    stepView = 'code';
    showFunction(k);
  }, true);
  P('sv-all').onclick = () => { stepView = 'all'; showFunction(k); };
  if (P('errtoggle')) P('errtoggle').onclick = () => { showErrors = !errorsShown; showFunction(k); };
  if (P('asidebtn')) P('asidebtn').onclick = () => { state.aside.includes(k) ? putBack(k) : takeOut(k); showFunction(k); };
  if (P('focusbtn')) P('focusbtn').onclick = () => setFocus(state.focus === k ? null : k);
  if (P('hidebtn')) P('hidebtn').onclick = () => (hidden.has(k) ? restoreFn(k) : hideFn(k));
  panel.querySelectorAll('.fchip').forEach((c) => (c.onclick = () => select(c.dataset.k, true)));
  panel.scrollTop = 0;
}

// ------------------------------------------------------------ telling apart

/// Several functions can read the same — `Fmt`, `New`, `Imp`, or one doc
/// sentence on two methods. Where a label is shared, lists add what tells
/// them apart: the type and name, and the file and line when even that
/// repeats (a `cfg(unix)` and a `cfg(windows)` version of one function).
let labelCounts = null, nameCounts = null;
function distinguisher(k, noWhere) {
  if (!labelCounts) {
    labelCounts = {}; nameCounts = {};
    for (const x of KEYS) {
      labelCounts[F[x].l] = (labelCounts[F[x].l] || 0) + 1;
      nameCounts[fullName(F[x])] = (nameCounts[fullName(F[x])] || 0) + 1;
    }
  }
  const f = F[k];
  if (labelCounts[f.l] < 2) return '';
  const name = fullName(f);
  const where = nameCounts[name] > 1 && !noWhere ? ' · ' + (shortFile(f.f) || f.f.split('/').pop()) + ':' + f.ln : '';
  return name + where;
}

/// A function's label for lists, with what tells it apart when it is shared.
function nameHTML(k, noWhere) {
  const d = distinguisher(k, noWhere);
  return labelHTML(F[k]) + (d ? ' <span class="ctx mono" title="' + esc(F[k].f + ':' + F[k].ln) + '">' + esc(d) + '</span>' : '');
}

// ------------------------------------------------------------ impact
//
// On a large change, what to review first. A change matters more the more
// features run through it (reach), and the riskier it is: a removed function,
// different inputs, new ways to crash or fail, many steps changed. Each
// change is scored from both, with the reasons said in words.

let impacts = null;
function impactOf(k) {
  if (!impacts) {
    impacts = {};
    const reach = {};
    for (const ft of DATA.features) {
      if (ft.s === 'same' && !ft.ci.length) continue;
      for (const c of ft.ci) reach[c] = (reach[c] || 0) + 1;
      if (ft.s !== 'same') reach[ft.k] = (reach[ft.k] || 0) + 1;
    }
    const stepsChanged = (rows) => rows.reduce((n, r) => n + (r.m !== 'same' ? 1 : 0) + stepsChanged(r.c), 0);
    for (const x of KEYS) {
      const f = F[x];
      if (f.s === 'same' || f.t) continue;
      let risk = 0;
      const why = [];
      if (f.s === 'removed') { risk += 4; why.push('removed'); }
      if (f.s === 'added') { risk += 1; why.push('new'); }
      if (f.sig) { risk += 3; why.push('takes different inputs'); }
      if (f.s === 'changed') {
        const now = endings(f.rows, 'a'), was = endings(f.rows, 'b');
        const crash = (now.crash + now.todo) - (was.crash + was.todo), fail = now.fail - was.fail;
        if (crash > 0) { risk += 3 * crash; why.push('+' + plural(crash, 'way to crash', 'ways to crash')); }
        if (fail > 0) { risk += 2 * fail; why.push('+' + plural(fail, 'error exit', 'error exits')); }
        const n = stepsChanged(f.rows);
        if (n) { risk += Math.min(3, Math.ceil(n / 3)); why.push(plural(n, 'step changed', 'steps changed')); }
        if (f.d) { risk += 1; why.push('details changed'); }
      }
      risk = Math.max(risk, 1);
      const r = reach[x] || 0;
      if (r > 1) why.unshift('reaches ' + r + ' features');
      impacts[x] = { score: risk * (1 + Math.log2(1 + r)), why, reach: r };
    }
  }
  return impacts[k] || { score: 0, why: [], reach: 0 };
}

/// How the Summary orders its lists: by impact (the default) or by file.
let summaryOrder = 'impact';

function summaryHTML() {
  const by = (s) => DATA.features.filter((f) => f.s === s);
  const changedFns = KEYS.filter((k) => onMap(k) && F[k].s !== 'same');
  const order = { changed: 0, added: 1, removed: 2 };
  const byFile = (a, b) => (F[a].f < F[b].f ? -1 : F[a].f > F[b].f ? 1 : F[a].ln - F[b].ln);
  if (summaryOrder === 'impact') changedFns.sort((a, b) => impactOf(b).score - impactOf(a).score || byFile(a, b));
  else changedFns.sort((a, b) => byFile(a, b) || order[F[a].s] - order[F[b].s]);
  const touched = DATA.features.filter((f) => f.s !== 'same');
  const featScore = (ft) => ft.ci.reduce((n, c) => n + impactOf(c).score, 0) + impactOf(ft.k).score;
  if (summaryOrder === 'impact') touched.sort((a, b) => featScore(b) - featScore(a) || byFile(a.k, b.k));
  else touched.sort((a, b) => byFile(a.k, b.k));
  // The few to look at first: the top of the list, while it stands out.
  const top = summaryOrder === 'impact' && changedFns.length > 8
    ? new Set(changedFns.slice(0, Math.max(3, Math.ceil(changedFns.length * 0.05))).filter((k) => impactOf(k).score >= impactOf(changedFns[0]).score * 0.35))
    : new Set();
  let h = '';
  if (!DATA.history || PLAIN) {
    const n = KEYS.filter(onMap).length;
    const shown = DATA.features.filter((ft) => onMap(ft.k) && (calls(ft.k).length || F[ft.k].rows.length));
    h += '<p class="summary"><b>The project as it is.</b> ' + plural(shown.length, 'feature', 'features') + ', ' +
      plural(n, 'function', 'functions') + '.</p>' + (PLAIN
        ? '<p class="sub">The code as it is now, compared with nothing. Switch to <b>Changes</b> to see what changed since ' + esc(againstShort()) + '. '
        : '<p class="sub">There is no earlier version to compare with, so nothing is marked as changed. ') +
      'Each box on the left is a feature — where the program starts doing something. Click any box to see its steps; +N shows what it calls.</p>' +
      (PLAIN ? '' : '<p class="sub" style="margin-top:8px">Once the project is committed to git, run this again after a change and the map will show what changed.</p>');
    h += '<div class="h3"><span>Features</span></div><ul class="list">' + shown.map((ft) =>
      '<li data-k="' + esc(ft.k) + '"><span class="lbl">' + nameHTML(ft.k) + '</span><span class="where mono">' + esc(fullName(F[ft.k])) + '</span></li>').join('') + '</ul>';
    return h;
  }
  if (!changedFns.length && !DATA.tests.length) {
    h += '<p class="summary"><b>Nothing changed.</b></p><p class="sub">Every feature works the same as before. Click any box on the map to see its steps.</p>';
  } else {
    h += '<p class="summary"><b>' + plural(changedFns.length, 'function', 'functions') + ' changed</b>, affecting ' +
      plural(touched.length, 'feature', 'features') + '. ' + plural(by('same').length, 'feature works', 'features work') + ' the same.</p>' +
      '<p class="sub">Click a change to see it on the map and step by step.</p>';
    h += '<div class="seg mini sortseg" role="group" aria-label="Order the lists">' +
      '<button data-sort="impact" class="' + (summaryOrder === 'impact' ? 'on' : '') + '" title="What to review first: how many features a change reaches, and how risky it is">By impact</button>' +
      '<button data-sort="file" class="' + (summaryOrder === 'file' ? 'on' : '') + '" title="In file and line order">By file</button></div>';
  }
  if (touched.length) {
    h += '<div class="h3"><span>Features affected</span><span class="hint">' + plural(touched.length, 'feature', 'features') + '</span></div><ul class="list" id="featlist">' + touched.map((ft) =>
      '<li data-k="' + esc(ft.k) + '"><span class="dot ' + ft.s + '"></span><span class="lbl">' + nameHTML(ft.k) +
      '</span><span class="where">' + (ft.ci.length ? plural(ft.ci.length, 'change', 'changes') + ' inside' : STATUS_WORD[ft.s]) + '</span></li>').join('') + '</ul>';
  }
  if (changedFns.length) {
    h += '<div class="h3"><span>Every change</span><span class="hint">' + (summaryOrder === 'impact' ? 'most important first' : 'in file order') + '</span></div><ul class="list chglist" id="chglist">' + changedFns.map((k) => {
      const im = impactOf(k);
      return '<li data-k="' + esc(k) + '"' + (top.has(k) ? ' data-changed="1"' : '') + '><span class="dot ' + F[k].s + '"></span><span class="lbl">' +
        (top.has(k) ? '<span class="first" title="High reach or risk: a good place to start">review first</span> ' : '') + nameHTML(k, true) +
        (im.why.length ? '<span class="why">' + esc(im.why.join(' · ')) + '</span>' : '') +
        '</span><span class="where mono" title="' + esc(F[k].f + ':' + F[k].ln) + '">' + esc(whereShort(F[k], 40)) + '</span></li>';
    }).join('') + '</ul>';
  }
  if (DATA.tests.length) {
    h += '<div class="h3"><span>Tests edited</span></div><p class="sub">A test edited alongside the code can hide a feature that broke.</p><ul class="list" id="testlist">' +
      DATA.tests.map((k) => '<li data-k="' + esc(k) + '"><span class="dot ' + F[k].s + '"></span><span class="lbl">' + nameHTML(k) +
      '</span><span class="where mono">' + esc(F[k].f) + ':' + F[k].ln + '</span></li>').join('') + '</ul>';
  }
  return h;
}

// ------------------------------------------------------------ back

/// Where the details panel has been: the overview, a function, a glossary
/// entry. Back returns to the one before; re-drawing the same place (a tab,
/// a step view) is not a new place.
const nav = { stack: [], cur: null, restoring: false };
const sameView = (a, b) => !!a && !!b && a.t === b.t && a.k === b.k;

function noteView(v) {
  if (!nav.restoring && nav.cur && !sameView(nav.cur, v)) {
    nav.stack.push(nav.cur);
    if (nav.stack.length > 60) nav.stack.shift();
  }
  nav.cur = v;
  syncBox();
}

function viewName(v) {
  if (v.t === 'fn') return F[v.k] ? F[v.k].l : 'the function';
  if (v.t === 'entry') return G[v.k] ? G[v.k].n : 'the entry';
  return 'Overview';
}

/// "← Back to Load settings" and, away from the overview, "Overview".
function navBar() {
  let prev = nav.stack[nav.stack.length - 1];
  const onOverview = nav.cur && nav.cur.t === 'overview';
  // In the box over the map the overview is not shown, so there is no going
  // back to it; ✕ closes the box instead.
  const inBox = !panelOpen && dock !== 'window';
  if (inBox && prev && prev.t === 'overview') prev = null;
  if (!prev && onOverview) return '';
  return '<div class="navbar">' +
    (prev ? '<button class="back" id="navback" title="Back (Alt+←)">← Back to <b>' + esc(viewName(prev)) + '</b></button>' : '') +
    (inBox ? '<button class="navclose" id="navclose" title="Close (Esc)" aria-label="Close">✕</button>'
      : !onOverview ? '<button class="navhome" id="navhome" title="The overview: what changed, and the glossary">Overview</button>' : '') +
    '</div>';
}

function goBack() {
  const v = nav.stack.pop();
  if (!v) return;
  nav.restoring = true;
  try {
    if (v.t === 'fn' && F[v.k]) select(v.k, true);
    else if (v.t === 'entry' && G[v.k]) showEntry(v.k, null);
    else select(null, false);
  } finally {
    nav.restoring = false;
  }
}

function wireNav() {
  const b = P('navback'), h = P('navhome');
  if (b) b.onclick = goBack;
  if (h) h.onclick = () => select(null, false);
  const c = P('navclose');
  if (c) c.onclick = () => select(null, false);
}

/// Which overview tab is open, and how far each was scrolled, so coming back
/// from a function or an entry lands where the reader left.
let overviewTab = 'summary';
const overviewScroll = {};

/// The overview: what changed, the project's structure, its glossary.
function showOverview() {
  const tabs = [['summary', 'Summary', 'What changed, and the features it touches'],
    ['glossary', 'Glossary', 'Every type, constant and static']];
  noteView({ t: 'overview' });
  let h = '<div class="pad">' + navBar() + '<div class="seg ovtabs" role="tablist">' + tabs.map(([id, name, tip]) =>
    '<button data-tab="' + id + '" class="' + (overviewTab === id ? 'on' : '') + '" title="' + tip + '">' + name + '</button>').join('') + '</div>';
  h += overviewTab === 'glossary' ? glossaryHTML() : summaryHTML();
  h += '</div>';
  const panel = P('panel');
  panel.innerHTML = h;
  panel.querySelectorAll('.ovtabs button').forEach((b) => (b.onclick = () => {
    overviewScroll[overviewTab] = panel.scrollTop;
    overviewTab = b.dataset.tab;
    showOverview();
  }));
  // Leaving for a function or an entry remembers where the list was.
  const leave = () => (overviewScroll[overviewTab] = panel.scrollTop);
  panel.querySelectorAll('li[data-k]').forEach((li) => (li.onclick = (ev) => { ev.stopPropagation(); leave(); select(li.dataset.k, true); }));
  panel.querySelectorAll('[data-g]').forEach((el) => el.addEventListener('click', leave, true));
  panel.querySelectorAll('.sortseg button').forEach((b) => (b.onclick = () => { summaryOrder = b.dataset.sort; showOverview(); }));
  ['featlist', 'chglist', 'testlist'].forEach((id) => paginate(panel.querySelector('#' + id), 'li'));
  wireGlossaryList();
  wireNav();
  panel.scrollTop = overviewScroll[overviewTab] || 0;
}

// ------------------------------------------------------------ structure

/// Folders opened or closed by the reader, by path; anything not in here
/// opens by default when it holds a change, or sits near the top.
const treeOpen = new Map();

/// The project as a tree: folders, files, and what each file defines in the
/// order it defines it — functions, methods under their type, structs,
/// enums, constants. Each item goes to its details.
function structureHTML() {
  const files = {};
  const add = (file, item) => (files[file] = files[file] || []).push(item);
  for (const k of KEYS) if (!(PLAIN && F[k].gone)) add(F[k].f, { fn: k, line: F[k].ln, owner: F[k].o, s: F[k].s });
  for (const k of GKEYS) if (!(PLAIN && G[k].gone)) add(G[k].f, { g: k, line: G[k].ln, s: G[k].s });

  // Folders from the paths.
  const root = { dirs: {}, files: [] };
  for (const path of Object.keys(files).sort()) {
    const parts = path.split('/');
    let node = root;
    for (const d of parts.slice(0, -1)) node = node.dirs[d] = node.dirs[d] || { dirs: {}, files: [] };
    node.files.push(path);
  }
  const changes = (items) => items.filter((it) => it.s !== 'same').length;
  const countIn = (node) => node.files.reduce((n, f) => n + changes(files[f]), 0) +
    Object.values(node.dirs).reduce((n, d) => n + countIn(d), 0);
  const fileCount = Object.keys(files).length;
  const isOpen = (path, has, depth) => treeOpen.has(path) ? treeOpen.get(path) : has > 0 || depth < 1 || fileCount <= 6;
  const tag = (n) => n ? '<span class="cnt">' + plural(n, 'change', 'changes') + '</span>' : '';

  const item = (it) => {
    if (it.fn) {
      const f = F[it.fn];
      return '<li class="ti' + (hidden.has(it.fn) ? ' hiddenfn' : '') + '" data-k="' + esc(it.fn) + '" data-text="' + esc((f.n + ' ' + f.l).toLowerCase()) + '" title="' + esc(fullName(f) + ' — line ' + f.ln) + '">' +
        (it.s !== 'same' ? '<span class="dot ' + it.s + '"></span>' : '<span class="nodot"></span>') +
        '<span class="gk fnk">' + (f.o ? 'method' : 'fn') + '</span><span class="lbl"><span class="mono">' + esc(f.n) + '</span>' +
        (f.l !== f.n ? ' <span class="gdesc">' + esc(f.l) + '</span>' : '') + '</span>' +
        (f.ft ? '<span class="tagx feat">feature</span>' : '') + (f.t ? '<span class="tagx">test</span>' : '') + '</li>';
    }
    const e = G[it.g];
    return '<li class="ti" data-g="' + esc(it.g) + '" data-text="' + esc(e.n.toLowerCase()) + '" title="' + esc(KIND_WORD[e.k] + ' — line ' + e.ln) + '">' +
      (it.s !== 'same' ? '<span class="dot ' + it.s + '"></span>' : '<span class="nodot"></span>') +
      '<span class="gk">' + e.k + '</span><span class="lbl"><span class="mono">' + esc(e.n) + '</span></span></li>';
  };

  const fileHTML = (path, depth) => {
    const items = files[path].slice().sort((a, b) => a.line - b.line);
    // Methods sit under their type, where the type's first method is.
    const out = [], impls = {};
    for (const it of items) {
      if (it.fn && it.owner) {
        if (!impls[it.owner]) { impls[it.owner] = []; out.push({ impl: it.owner }); }
        impls[it.owner].push(it);
      } else out.push(it);
    }
    const n = changes(items);
    const name = path.split('/').pop();
    return '<details class="tfile" data-path="' + esc(path) + '"' + (isOpen(path, n, depth) ? ' open' : '') + '>' +
      '<summary><span class="tw"></span><span class="mono">' + esc(name) + '</span><span class="tn">' + items.length + '</span>' + tag(n) + '</summary>' +
      '<ul class="titems">' + out.map((it) => it.impl
        ? '<li class="timpl"><span class="kw">impl</span> <span class="mono">' + esc(it.impl) + '</span><ul class="titems">' + impls[it.impl].map(item).join('') + '</ul></li>'
        : item(it)).join('') + '</ul></details>';
  };

  const dirHTML = (node, path, depth) => Object.keys(node.dirs).sort().map((d) => {
    const p = path ? path + '/' + d : d, sub = node.dirs[d], n = countIn(sub);
    return '<details class="tdir" data-path="' + esc(p) + '"' + (isOpen(p, n, depth) ? ' open' : '') + '>' +
      '<summary><span class="tw"></span><span class="mono">' + esc(d) + '/</span>' + tag(n) + '</summary>' +
      '<div class="tkids">' + dirHTML(sub, p, depth + 1) + sub.files.map((f) => fileHTML(f, depth + 1)).join('') + '</div></details>';
  }).join('');

  const total = KEYS.length + GKEYS.length;
  return '<div class="h3 treehead"><span>Project structure</span><span class="hint">' + plural(fileCount, 'file', 'files') + ' · ' +
    plural(total, 'item', 'items') + '</span></div>' +
    '<input id="tfilter" class="gfilter" type="search" placeholder="Find a function, type or file…" autocomplete="off">' +
    '<div class="tree" id="tree">' + dirHTML(root, '', 0) + root.files.map((f) => fileHTML(f, 0)).join('') + '</div>';
}

/// Mark the selected function in the structure tree.
function markTree() {
  const tree = $('tree');
  if (!tree) return;
  tree.querySelectorAll('li.ti.cur').forEach((li) => li.classList.remove('cur'));
  const li = state.selected && tree.querySelector('li.ti[data-k="' + CSS.escape(state.selected) + '"]');
  if (!li) return;
  li.classList.add('cur');
  // Open the folders around it and bring it into view.
  for (let d = li.closest('details'); d; d = d.parentElement && d.parentElement.closest('details')) d.open = true;
  li.scrollIntoView({ block: 'nearest' });
}

function wireStructure(root) {
  const tree = root.querySelector('#tree'), filter = root.querySelector('#tfilter');
  if (!tree) return;
  tree.querySelectorAll('li[data-k]').forEach((li) => li.addEventListener('click', (ev) => { ev.stopPropagation(); select(li.dataset.k, true); }));
  markTree();
  tree.querySelectorAll('details').forEach((d) => d.addEventListener('toggle', () => {
    if (!tree.classList.contains('filtering')) treeOpen.set(d.dataset.path, d.open);
  }));
  tree.querySelectorAll('li[data-g]').forEach((li) => li.addEventListener('click', (ev) => { ev.stopPropagation(); showEntry(li.dataset.g, null); }));
  filter.addEventListener('input', () => {
    const q = filter.value.trim().toLowerCase();
    tree.classList.toggle('filtering', !!q);
    tree.querySelectorAll('li.ti').forEach((li) => (li.hidden = !!q && !li.dataset.text.includes(q)));
    tree.querySelectorAll('li.timpl').forEach((li) => (li.hidden = !!q && !li.querySelector('li.ti:not([hidden])')));
    // Files match by name too; folders show while anything inside does.
    tree.querySelectorAll('details.tfile').forEach((d) => {
      const byName = d.dataset.path.toLowerCase().includes(q);
      if (byName && q) d.querySelectorAll('li.ti, li.timpl').forEach((li) => (li.hidden = false));
      d.hidden = !!q && !byName && !d.querySelector('li.ti:not([hidden])');
      if (q) d.open = true;
    });
    [...tree.querySelectorAll('details.tdir')].reverse().forEach((d) => {
      d.hidden = !!q && !d.querySelector('details.tfile:not([hidden])');
      if (q) d.open = true;
    });
    if (!q) renderSide();
  });
}

/// Past this many boxes the whole map is too dense to read, so selecting a
/// box collapses the map around it. Below it, the map stays put.
const BIG_MAP = 40;

/// Select a function: the panel shows its steps, and the map highlights it,
/// what it calls, and the path to it — in place, without rearranging. The map
/// collapses around it only when the map is too big to read whole, or when
/// it is already collapsed and the reader is walking along the flow.
/// `null` clears the selection and any collapse.
function select(k, center, from) {
  const was = state.selected;
  state.selected = k;
  // The details box over the map opens now, so the map can keep the
  // selected box clear of it.
  if (k) document.querySelector('.split').classList.add('modal-on');
  // Somewhere in the flow in the side space: it is shown there, not
  // dragged back into the main map.
  const inSide = !!(side && flowShown() && k && pathWithin(state.asideOn, k).length);
  let focus = state.focus;
  if (!k) focus = null;
  // A click in the side space never collapses the main map.
  else if (onMap(k) && from !== side && (state.focus || Object.keys(main.placed).length > BIG_MAP)) focus = k;

  if (focus !== state.focus || (k && onMap(k) && !main.placed[k] && !inSide)) {
    if (focus !== state.focus) swapUnfolds(focus);
    state.focus = focus;
    main.draw(true);
    if (side && flowShown()) side.focusNear();
  } else {
    main.focusNear();
    if (side && flowShown()) {
      // Bring a selection deep inside the taken-out flow into the side map.
      if (inSide && !side.placed[k]) side.draw(false);
      side.focusNear();
    }
    if (k) {
      if (from === side || (inSide && !main.placed[k])) side.reveal(k);
      else if (main.placed[k]) main.reveal(k);
    }
  }
  if (k) {
    showFunction(k);
  } else {
    showOverview();
  }
  markTree();
  if (k !== was) {
    try { history.replaceState(null, '', k ? '#' + encodeURIComponent(k) : location.pathname); } catch (e) {}
  }
}

/// The +N unfolds made on the whole map are put aside while it is collapsed
/// and restored when it comes back; a collapsed view starts with none.
let wholeMapUnfolds = new Set();
function swapUnfolds(nextFocus) {
  if (!state.focus && nextFocus) { wholeMapUnfolds = main.expanded; main.expanded = new Set(); }
  else if (state.focus && !nextFocus) { main.expanded = wholeMapUnfolds; wholeMapUnfolds = new Set(); }
  else main.expanded = new Set();
}

/// Collapse the map around `k` on request, or bring everything back.
function setFocus(k) {
  swapUnfolds(k);
  state.focus = k;
  main.draw(true);
  if (state.selected) showFunction(state.selected);
}

// ------------------------------------------------------------ search

// ------------------------------------------------------------ glossary

const G = DATA.glossary || {};
const GKEYS = Object.keys(G);
for (const k of GKEYS) G[k].key = k;
/// Name → entry, for linking type names wherever they are written. A name
/// defined twice links to neither rather than to the wrong one.
const GNAME = {};
for (const k of GKEYS) GNAME[G[k].n] = GNAME[G[k].n] === undefined ? k : null;
const KIND_WORD = { struct: 'Structure', enum: 'Choice (enum)', trait: 'Trait', type: 'Type alias', const: 'Constant', static: 'Static value' };

/// A doc comment as a few paragraphs, `code` kept as code.
function docHTML(doc) {
  if (!doc) return '';
  return doc.split(/\n\s*\n/).map((para) => '<p>' + esc(para.replace(/\n/g, ' '))
    .replace(/`([^`]+)`/g, '<code>$1</code>') + '</p>').join('');
}

/// Code with every glossary name in it turned into a link to its entry.
function linkTypes(text) {
  const html = esc(text);
  return html.replace(/\b([A-Z][A-Za-z0-9_]*|[A-Z][A-Z0-9_]+)\b/g, (m) =>
    GNAME[m] ? '<a class="glink" data-g="' + esc(GNAME[m]) + '" title="' + esc(KIND_WORD[G[GNAME[m]].k] + ' — click for its glossary entry') + '">' + m + '</a>' : m);
}

function gChip(k) {
  const e = G[k];
  if (!e) return '';
  return '<button class="gchip ' + e.s + '" data-g="' + esc(k) + '" title="' + esc(KIND_WORD[e.k] + (e.doc ? ' — ' + e.doc.split('\n')[0] : '')) + '">' +
    '<span class="gk">' + e.k + '</span> ' + esc(e.n) + (e.s !== 'same' ? ' · ' + STATUS_WORD[e.s].toLowerCase() : '') + '</button>';
}

/// A glossary entry in a few words: its kind, name, first line of doc, and
/// its shape — fields, choices, or type and value.
function gCard(k) {
  const e = G[k];
  const shape = e.parts.length
    ? e.parts.slice(0, 4).map(([n, t]) => esc(n) + (t && e.k !== 'enum' ? ': ' + linkTypes(t) : '')).join(' · ') + (e.parts.length > 4 ? ' · +' + (e.parts.length - 4) : '')
    : e.ty ? linkTypes(e.ty) + (e.val ? ' = ' + cut(e.val, 50) : '') : '';
  const first = e.doc ? e.doc.split(/\n\s*\n/)[0].replace(/\n/g, ' ') : '';
  return '<div class="gcard ' + e.s + '" data-g="' + esc(k) + '"' + (e.s !== 'same' ? ' data-changed="1"' : '') + ' title="Open its glossary entry">' +
    '<div class="gc-head"><span class="gk">' + e.k + '</span> <span class="mono gc-name">' + esc(e.n) + '</span>' +
    (e.s !== 'same' ? ' <span class="pill ' + e.s + '">' + STATUS_WORD[e.s] + '</span>' : '') + '</div>' +
    (first ? '<div class="gc-doc">' + cut(first, 120) + '</div>' : '') +
    (shape ? '<div class="gc-shape mono">' + shape + '</div>' : '') + '</div>';
}

/// What a variable starts as, in words where its value is chosen rather than
/// computed: `match x { … }` reads "chosen by matching x".
function startsAs(init) {
  const lnk = (t) => quoted(t, init);
  const m = /^match\s+(.+?)\s*\{/.exec(init);
  if (m) return '<span class="kw">chosen by matching</span> ' + lnk(m[1]);
  const i = /^if\s+(.+?)\s*\{/.exec(init);
  if (i) return '<span class="kw">depends on</span> ' + lnk(i[1]);
  return lnk(init);
}

/// Where a variable changes, the same change on several lines said once:
/// "→ false at lines 36, 45, 54".
function changesOf(sets) {
  const groups = [];
  for (const [l, op, val] of sets) {
    const g = groups.find((x) => x.op === op && x.val === val);
    if (g) g.lines.push(l); else groups.push({ op, val, lines: [l] });
  }
  const lineLink = (l) => '<a class="lnref" data-ln="' + l + '" title="Open the code at this line">' + l + '</a>';
  return groups.map((g) => '<div class="vset">' + (g.op === '=' ? '→' : esc(g.op)) + ' ' + quoted(g.val) +
    ' <span class="kw">at line' + (g.lines.length > 1 ? 's' : '') + '</span> ' + g.lines.map(lineLink).join(', ') + '</div>').join('');
}

/// In the steps and the code, a variable or a value from this function's
/// glossary is a link: clicking it shows its row (or card) in the glossary,
/// turning the page if it is on another one. In the code, a call to one of
/// the user's functions opens that function too. Steps drawn later — an
/// unfolded branch, a "+N steps" — are linked as they appear.
function linkStepNames(fc, panel, k) {
  const rows = [...panel.querySelectorAll('table.vars tr.vrow')];
  const cards = [...panel.querySelectorAll('.gcards .gcard')];
  const vars = new Set(rows.map((r) => r.dataset.v));
  const values = new Map(cards.map((c) => [G[c.dataset.g] ? G[c.dataset.g].n : '', c]).filter(([n]) => n));
  // The functions it calls, by the name the code uses for them.
  const fns = new Map();
  for (const c of [...(F[k].ce || []), ...(F[k].cbe || [])]) if (F[c] && !fns.has(F[c].n)) fns.set(F[c].n, c);
  const names = [...new Set([...vars, ...values.keys(), ...fns.keys()])].sort((a, b) => b.length - a.length);
  if (!names.length) return;
  const re = new RegExp('\\b(' + names.map((n) => n.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|') + ')\\b', 'g');

  // Quoted code in the steps, and the source lines of the code view — but
  // not its strings and comments.
  const link = (root) => [...root.querySelectorAll('code'), ...root.querySelectorAll('.code td.s')].forEach((codeEl) => {
    if ((codeEl.tagName === 'CODE' && codeEl.closest('.code')) || codeEl.querySelector('.vref, .fref')) return;
    const walker = codeEl.ownerDocument.createTreeWalker(codeEl, NodeFilter.SHOW_TEXT);
    const texts = [];
    while (walker.nextNode()) if (!walker.currentNode.parentNode.closest('.c-str, .c-com')) texts.push(walker.currentNode);
    for (const t of texts) {
      if (!re.test(t.data)) { re.lastIndex = 0; continue; }
      re.lastIndex = 0;
      const frag = codeEl.ownerDocument.createDocumentFragment();
      let last = 0, m;
      while ((m = re.exec(t.data))) {
        // After a dot it is a method or a field: only one of the user's
        // methods is linked there — `data.get(..)` is not their `get`.
        const dotted = t.data.slice(0, m.index).trimEnd().endsWith('.');
        const fnHere = fns.get(m[1]);
        if (dotted && !(fnHere && F[fnHere].o)) continue;
        frag.append(t.data.slice(last, m.index));
        const a = codeEl.ownerDocument.createElement('a');
        const fn = dotted ? fnHere : !vars.has(m[1]) && !values.has(m[1]) ? fnHere : null;
        a.className = fn ? 'fref' : 'vref';
        a.dataset.name = m[1];
        if (fn) a.dataset.k = fn;
        a.textContent = m[1];
        a.title = fn ? 'Open ' + fullName(F[fn]) : vars.has(m[1]) ? 'Variable — show where it starts and changes' : 'Show it in this function’s glossary';
        frag.append(a);
        last = m.index + m[1].length;
      }
      frag.append(t.data.slice(last));
      t.replaceWith(frag);
    }
  });

  const go = (a) => {
    const name = a.dataset.name;
    let target;
    if (vars.has(name)) {
      // The declaration in effect at this step or line: the last one on or
      // above it.
      const holder = a.closest('.st, .mp-step, .mp-exit');
      const row = a.closest('.code tr');
      const at = row ? +(row.dataset.a || row.dataset.b || Infinity)
        : holder && holder.querySelector('[data-ln]') ? +holder.querySelector('[data-ln]').dataset.ln : Infinity;
      const same = rows.filter((r) => r.dataset.v === name);
      target = same.filter((r) => +r.dataset.vln <= at).pop() || same[0];
      const table = panel.querySelector('table.vars');
      if (table.showItem) table.showItem(target);
    } else {
      target = values.get(name);
      const box = panel.querySelector('.gcards');
      if (box && box.showItem) box.showItem(target);
    }
    if (!target) return;
    target.scrollIntoView({ block: 'center', behavior: CALM ? 'auto' : 'smooth' });
    target.classList.remove('flash');
    void target.offsetWidth;
    target.classList.add('flash');
  };
  fc.addEventListener('click', (e) => {
    const f = e.target.closest('a.fref');
    if (f) { e.stopPropagation(); e.preventDefault(); select(f.dataset.k, true); return; }
    const a = e.target.closest('a.vref');
    if (!a) return;
    e.stopPropagation();
    e.preventDefault();
    go(a);
  }, true);
  link(fc);
  new MutationObserver((list) => list.forEach((m) => m.addedNodes.forEach((n) => { if (n.nodeType === 1) link(n); }))).observe(fc, { childList: true, subtree: true });
}

/// The function's own glossary: its variables — where each starts and
/// where it changes — and the types and values it touches.
function fnGlossaryHTML(f) {
  const vars = f.v || [];
  // Types named in its signature or its variables count as touched too.
  const named = new Set(f.u || []);
  const scan = (t) => (t || '').replace(/\b[A-Z][A-Za-z0-9_]*\b/g, (m) => { if (GNAME[m]) named.add(GNAME[m]); return m; });
  f.p.forEach(([, t]) => scan(t));
  scan(f.r);
  vars.forEach((v) => scan(v.t));
  const entries = [...named].filter((k) => G[k]);
  if (!vars.length && !entries.length) return '';
  let h = '<div class="h3"><span>Glossary</span><span class="hint">for this function</span></div>';
  if (vars.length) {
    h += '<div class="gsub">Variables</div><table class="vars"><tr><th>Name</th><th>Type</th><th>Starts as</th><th>Changes</th></tr>' +
      vars.map((v) => '<tr class="vrow" data-v="' + esc(v.n) + '" data-vln="' + v.ln + '"><td class="mono vn">' + esc(v.n) + (v.m ? ' <span class="tagx">mut</span>' : '') +
        '<div class="vline"><a class="lnref" data-ln="' + v.ln + '" title="Open the code at this line">line ' + v.ln + '</a></div></td>' +
        '<td class="mono">' + (v.t ? linkTypes(v.t) : '<span class="empty">—</span>') + '</td>' +
        '<td>' + (v.i ? startsAs(v.i) : '<span class="empty">set later</span>') + '</td>' +
        '<td>' + (v.s.length ? changesOf(v.s) : '<span class="empty">' + (v.m ? 'never reassigned' : 'fixed') + '</span>') + '</td></tr>').join('') + '</table>';
  }
  if (entries.length) {
    h += '<div class="gsub">Types and values it uses</div><div class="gcards">' + entries.map(gCard).join('') + '</div>';
  }
  return h;
}

/// Wire every glossary link and chip inside `root` to open its entry.
function wireGlossary(root, back) {
  root.querySelectorAll('[data-g]').forEach((el) => {
    el.addEventListener('click', (ev) => { ev.stopPropagation(); showEntry(el.dataset.g, back); });
  });
}

/// One glossary entry in the details panel. `back` is the function to
/// return to, when the reader came from one.
function showEntry(key, back) {
  const e = G[key];
  if (!e) return;
  const panel = P('panel');
  noteView({ t: 'entry', k: key });
  let h = '<div class="pad">' + navBar() +
    '<div><span class="pill ' + e.s + '">' + STATUS_WORD[e.s] + '</span> <span class="pill feature">' + esc(KIND_WORD[e.k]) + '</span></div>' +
    '<h2 class="mono">' + esc(e.n) + '</h2><div class="where mono">' + esc(e.f) + ':' + e.ln + '</div>';
  h += e.doc ? '<div class="doc">' + docHTML(e.doc) + '</div>' : '<p class="empty">No description in the code.</p>';
  if (e.ty) h += '<div class="rel"><b>' + (e.k === 'type' ? 'Stands for' : 'Type') + '</b><code>' + linkTypes(e.ty) + '</code></div>';
  if (e.val) h += '<div class="rel"><b>Value</b><code>' + esc(e.val) + '</code></div>';
  if (e.parts.length) {
    const head = { struct: 'Fields', enum: 'Choices', trait: 'Requires' }[e.k] || 'Parts';
    h += '<div class="h3"><span>' + head + '</span></div><table class="parts">' + e.parts.map(([n, t, d]) =>
      '<tr><td class="mono pn">' + esc(n) + '</td><td class="mono pt">' + (t ? linkTypes(t) : '') + '</td><td class="pd">' + (d ? docHTML(d) : '') + '</td></tr>').join('') + '</table>';
  }
  h += '<div class="rel"><b>Used by</b>' + (e.ub.length ? chipList(e.ub.filter((k) => F[k])) : '<span class="empty">no function of yours mentions it</span>') + '</div>';
  h += '<div class="h3"><span>Definition</span></div>';
  if (e.s === 'changed' && e.srcb) {
    h += '<p class="code-note">It changed. Before:</p><div class="code"><pre>' + highlightBlock(e.srcb) + '</pre></div><p class="code-note">Now:</p>';
  }
  h += '<div class="code"><pre>' + highlightBlock(e.src) + '</pre></div></div>';
  panel.innerHTML = h;
  wireNav();
  panel.querySelectorAll('.fchip').forEach((c) => (c.onclick = () => select(c.dataset.k, true)));
  wireGlossary(panel, back);
  panel.scrollTop = 0;
}

const highlightBlock = (src) => src.split('\n').map(highlight).join('\n');

/// The glossary list for the overview: every entry, filterable.
function glossaryHTML() {
  if (!GKEYS.length) return '';
  const order = { struct: 0, enum: 1, trait: 2, type: 3, const: 4, static: 5 };
  const keys = GKEYS.filter((k) => !(PLAIN && G[k].gone)).sort((a, b) => (order[G[a].k] - order[G[b].k]) || G[a].n.localeCompare(G[b].n));
  const changed = keys.filter((k) => G[k].s !== 'same').length;
  return '<div class="h3"><span>Glossary</span><span class="hint">' + plural(keys.length, 'entry', 'entries') +
    (changed ? ' · ' + changed + ' changed' : '') + '</span></div>' +
    '<input id="gfilter" class="gfilter" type="search" placeholder="Filter types, constants…" autocomplete="off">' +
    '<ul class="list glist" id="glist">' + keys.map((k) => {
      const e = G[k];
      const first = e.doc ? e.doc.split(/\n\s*\n/)[0].replace(/\n/g, ' ') : '';
      return '<li data-g="' + esc(k) + '" data-text="' + esc((e.n + ' ' + first).toLowerCase()) + '"' + (e.s !== 'same' ? ' data-changed="1"' : '') + '>' +
        (e.s !== 'same' ? '<span class="dot ' + e.s + '"></span>' : '') +
        '<span class="gk">' + e.k + '</span><span class="lbl"><span class="mono">' + esc(e.n) + '</span>' +
        (first ? ' <span class="gdesc">— ' + cut(first, 90) + '</span>' : '') +
        '</span></li>';
    }).join('') + '</ul>';
}

function wireGlossaryList() {
  const list = P('glist'), filter = P('gfilter');
  if (!list) return;
  wireGlossary(list, null);
  const restart = paginate(list, 'li');
  filter.addEventListener('input', () => {
    const q = filter.value.trim().toLowerCase();
    list.querySelectorAll('li').forEach((li) => {
      if (q && !li.dataset.text.includes(q)) li.dataset.off = '1';
      else delete li.dataset.off;
    });
    restart();
  });
}

/// How the search box matches. Plain text: found anywhere in a function's
/// names. A regular expression — with .* switched on, or written /like this/
/// — is tested against each name on its own (the function's name, its full
/// path, its label, its file), so `^get` and `_efile$` anchor to a name.
/// Case is ignored either way.
const REGEX_KEY = 'flowlens:search-regex';
let regexMode = (() => { try { return localStorage.getItem(REGEX_KEY) === '1'; } catch (e) { return false; } })();
let searcher = { test: () => false, name: () => false, error: null, regex: false };
const namesOf = (k) => [F[k].n, fullName(F[k]), F[k].l, F[k].f];

function compileSearch(raw) {
  const slashed = /^\/(.+)\/([a-z]*)$/.exec(raw);
  if (slashed || regexMode) {
    try {
      const flags = 'i' + (slashed ? slashed[2].replace(/[giy]/g, '') : '');
      const re = new RegExp(slashed ? slashed[1] : raw, flags);
      return { test: (k) => namesOf(k).some((n) => re.test(n)), name: (n) => re.test(n), error: null, regex: true };
    } catch (e) {
      return { test: () => false, name: () => false, error: e.message.replace(/^Invalid regular expression: /, ''), regex: true };
    }
  }
  const q = raw.toLowerCase();
  return { test: (k) => namesOf(k).join(' ').toLowerCase().includes(q), name: (n) => n.toLowerCase().includes(q), error: null, regex: false };
}

const matches = (k) => searcher.test(k);

/// Every place a matching function appears on the maps: its box and every
/// ↪ chip that stands for a call to it elsewhere — in the main map and in a
/// flow taken out to the side space — in reading order.
function searchPlaces(q) {
  const out = [];
  for (const m of [main, side]) {
    if (!m || m.wrap.hidden || (m === side && !flowShown())) continue;
    m.svgEl.querySelectorAll('.node[data-k], .proxy[data-k]').forEach((el) => {
      if (!el.classList.contains('pager') && matches(el.dataset.k, q)) out.push({ el, map: m });
    });
  }
  const at = (el) => {
    const r = /translate\(([-\d.]+)px,\s*([-\d.]+)px\)/.exec(el.style.transform || '') || [0, 0, 0];
    return [+r[1], +r[2]];
  };
  out.sort((a, b) => (a.map === b.map ? 0 : a.map === main ? -1 : 1) ||
    (state.dir === 'h' ? at(a.el)[0] - at(b.el)[0] || at(a.el)[1] - at(b.el)[1] : at(a.el)[1] - at(b.el)[1] || at(a.el)[0] - at(b.el)[0]));
  return out;
}

/// Where Enter is in the list of places, and which functions match but are
/// not on any map right now.
const search = { places: [], at: -1, offMap: [] };

/// Mark every place that matches and say how many there are.
function markSearch() {
  const q = state.query;
  document.querySelectorAll('.node.hit, .proxy.hit, .hit-now').forEach((el) => el.classList.remove('hit', 'hit-now'));
  const count = $('qcount');
  search.at = -1;
  if (!q) { search.places = []; search.offMap = []; count.hidden = true; return; }
  if (searcher.error) {
    search.places = []; search.offMap = [];
    count.hidden = false;
    count.textContent = 'invalid pattern';
    count.title = searcher.error;
    return;
  }
  search.places = searchPlaces(q);
  search.places.forEach((p) => p.el.classList.add('hit'));
  const shownKeys = new Set(search.places.map((p) => p.el.dataset.k));
  search.offMap = KEYS.filter((k) => onMap(k) && matches(k, q) && !shownKeys.has(k));
  const n = search.places.length, m = search.offMap.length;
  count.hidden = false;
  count.textContent = (n ? plural(n, 'place', 'places') : 'none on the map') + (m ? ' · +' + m + ' not on the map' : '');
  count.title = 'Enter goes to the next one, Shift+Enter to the one before' + (m ? '; after the map, the ones not on it' : '');
}

/// Go to the next (or previous) place: select its function and bring that
/// box or chip into view. Past the places on the map come the matches that
/// are not on it, which selecting brings onto the map.
function stepSearch(dir) {
  if (!state.query) return;
  const total = search.places.length + search.offMap.length;
  if (!total) {
    const entry = GKEYS.find((k) => !(PLAIN && G[k].gone) && searcher.name(G[k].n));
    if (entry) showEntry(entry, null);
    return;
  }
  search.at = (search.at + dir + total) % total;
  const count = $('qcount');
  if (search.at < search.places.length) {
    const p = search.places[search.at];
    document.querySelectorAll('.hit-now').forEach((el) => el.classList.remove('hit-now'));
    select(p.el.dataset.k, false, p.map);
    // Selecting may redraw; find the same place again by its function and kind.
    const again = searchPlaces(state.query);
    const same = again.find((x) => x.map === p.map && x.el.dataset.k === p.el.dataset.k && x.el.classList.contains('proxy') === p.el.classList.contains('proxy')) || again[search.at] || p;
    again.forEach((x) => x.el.classList.add('hit'));
    same.el.classList.add('hit-now');
    same.map.centerOnEl(same.el);
    search.places = again;
  } else {
    const k = search.offMap[search.at - search.places.length];
    select(k, true);
    markSearch();
    const i = search.places.findIndex((x) => x.el.dataset.k === k && !x.el.classList.contains('proxy'));
    search.at = i;
    if (i >= 0) search.places[i].el.classList.add('hit-now');
  }
  count.textContent = (search.at + 1) + ' of ' + (search.places.length + search.offMap.length);
}

// ------------------------------------------------------------ pattern helper
//
// Ready-made patterns to start from, the reader's own saved ones, and what
// the signs mean. A pattern's [word] is the part to change: it is selected
// after the pattern goes in, ready to type over.

const PATTERNS = [
  ['Starts with', '^[get]', 'names that begin with these letters'],
  ['Ends with', '[_efile]$', 'names that end with these letters'],
  ['Exactly', '^[main]$', 'one name, and nothing longer'],
  ['One of several', '^([add]|[remove])_', 'either word, then the rest'],
  ['A whole word', '\\b[job]\\b', 'the word on its own, not inside another'],
  ['Methods of a type', '[Workspace]::', 'everything defined in its impl'],
  ['In a file or folder', '[update_efile]/', 'everything defined there'],
  ['Tests', '^test_|::tests::', 'test functions'],
  ['Has a number', '\\d', 'names with a digit in them'],
];

const SIGNS_RX = [
  ['^', 'start of the name'], ['$', 'end of the name'], ['.', 'any one character'],
  ['*', 'the one before, any number of times'], ['+', 'the one before, once or more'],
  ['?', 'the one before, maybe'], ['a|b', 'either a or b'], ['( )', 'group: ^(get|set)_'],
  ['[abc]', 'one of these characters'], ['\\b', 'edge of a word'], ['\\d', 'a digit'],
  ['\\', 'next sign taken literally: \\.'],
];

const PATTERNS_KEY = 'flowlens:search-patterns';
const savedPatterns = () => { try { return JSON.parse(localStorage.getItem(PATTERNS_KEY)) || []; } catch (e) { return []; } };
const keepPatterns = (list) => { try { localStorage.setItem(PATTERNS_KEY, JSON.stringify(list)); } catch (e) {} };

/// Put a pattern in the search box with regex on. A ready pattern's [word]
/// is ours — the part to change — and is selected after it goes in; a saved
/// pattern (`literal`) goes in exactly as it was written.
function usePattern(p, literal) {
  const q = $('q');
  const m = literal ? null : /\[([A-Za-z_]+)\]/.exec(p);
  const text = literal ? p : p.replace(/\[([A-Za-z_]+)\]/g, '$1');
  if (!regexMode) $('regexbtn').click();
  q.value = text;
  q.dispatchEvent(new Event('input'));
  q.focus();
  if (m) {
    const at = p.slice(0, m.index).replace(/\[([A-Za-z_]+)\]/g, '$1').length;
    q.setSelectionRange(at, at + m[1].length);
  }
}

function renderPatternHelper() {
  const panel = $('rxpanel');
  const mine = savedPatterns();
  const current = $('q').value.trim();
  let h = '<div class="rx-head"><b>Search patterns</b><button class="rx-x" id="rxclose" title="Fold away (Esc)">✕</button></div>';
  h += '<div class="rx-sec">Ready patterns <span class="hint">click one, then type over the highlighted part</span></div><ul class="rx-list">' +
    PATTERNS.map(([name, pat, what], i) => '<li data-i="' + i + '"><span class="rx-name">' + esc(name) + '</span><code>' +
      esc(pat.replace(/\[([A-Za-z_]+)\]/g, '$1')) + '</code><span class="rx-what">' + esc(what) + '</span></li>').join('') + '</ul>';
  h += '<div class="rx-sec">Your patterns <span class="hint">kept in this browser</span></div>';
  h += mine.length ? '<ul class="rx-list mine">' + mine.map((p, i) => '<li data-m="' + i + '"><span class="rx-name">' + esc(p.name) +
    '</span><code>' + esc(p.pattern) + '</code><button class="rx-del" data-del="' + i + '" title="Remove this pattern">✕</button></li>').join('') + '</ul>'
    : '<p class="empty rx-empty">None yet — type a search, then save it here.</p>';
  h += '<div class="rx-save"><input id="rxname" type="text" placeholder="Name for the current search" autocomplete="off"' + (current ? '' : ' disabled') + '>' +
    '<button id="rxsave"' + (current ? '' : ' disabled') + ' title="Save the search box as one of your patterns">Save</button></div>';
  h += '<details class="rx-signs"' + (renderPatternHelper.signsOpen ? ' open' : '') + '><summary>What the signs mean</summary><table>' +
    SIGNS_RX.map(([sg, m]) => '<tr><td><code>' + esc(sg) + '</code></td><td>' + esc(m) + '</td></tr>').join('') + '</table></details>';
  panel.innerHTML = h;
  panel.querySelectorAll('li[data-i]').forEach((li) => (li.onclick = () => usePattern(PATTERNS[+li.dataset.i][1])));
  panel.querySelectorAll('li[data-m]').forEach((li) => (li.onclick = (e) => {
    if (e.target.closest('.rx-del')) return;
    const p = savedPatterns()[+li.dataset.m];
    if (p) usePattern(p.pattern, true);
  }));
  panel.querySelectorAll('.rx-del').forEach((b) => (b.onclick = (e) => {
    e.stopPropagation();
    const list = savedPatterns();
    list.splice(+b.dataset.del, 1);
    keepPatterns(list);
    renderPatternHelper();
  }));
  const save = () => {
    const pattern = $('q').value.trim();
    if (!pattern) return;
    const name = $('rxname').value.trim() || pattern;
    const list = savedPatterns().filter((p) => p.pattern !== pattern);
    list.push({ name, pattern });
    keepPatterns(list);
    renderPatternHelper();
  };
  $('rxsave').onclick = save;
  $('rxname').onkeydown = (e) => { if (e.key === 'Enter') save(); };
  $('rxclose').onclick = () => togglePatternHelper(false);
  panel.querySelector('.rx-signs').addEventListener('toggle', (e) => (renderPatternHelper.signsOpen = e.target.open));
}

function togglePatternHelper(open) {
  const panel = $('rxpanel'), btn = $('rxhelp');
  if (open === undefined) open = panel.hidden;
  if (open) { renderPatternHelper(); toggleSigns(false); }
  panel.hidden = !open;
  btn.classList.toggle('on', open);
  btn.setAttribute('aria-expanded', String(open));
}

function wireSearch() {
  $('rxhelp').onclick = () => togglePatternHelper();
  $('signsbtn').onclick = () => toggleSigns();
  // The save row follows what is in the box.
  $('q').addEventListener('input', () => { if (!$('rxpanel').hidden) {
    const has = !!$('q').value.trim();
    $('rxname').disabled = !has;
    $('rxsave').disabled = !has;
  } });
  const q = $('q');
  const refresh = () => {
    state.query = q.value.trim();
    searcher = compileSearch(state.query);
    q.classList.toggle('bad', !!searcher.error);
    q.classList.toggle('regex', searcher.regex);
    markSearch();
  };
  q.addEventListener('input', refresh);
  const rb = $('regexbtn');
  const showMode = () => {
    rb.classList.toggle('on', regexMode);
    rb.setAttribute('aria-pressed', String(regexMode));
    q.placeholder = regexMode ? 'Find — regular expression  /' : 'Find a function  /';
  };
  rb.onclick = () => {
    regexMode = !regexMode;
    try { localStorage.setItem(REGEX_KEY, regexMode ? '1' : '0'); } catch (e) {}
    showMode();
    refresh();
    q.focus();
  };
  showMode();
  q.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' || !state.query) return;
    e.preventDefault();
    stepSearch(e.shiftKey ? -1 : 1);
  });
  document.addEventListener('keydown', (e) => {
    if (e.altKey && e.key === 'ArrowLeft') { e.preventDefault(); goBack(); return; }
    if (e.key === '/' && document.activeElement !== q) { e.preventDefault(); q.focus(); }
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test((document.activeElement || {}).tagName || '');
    if ((e.key === 'Delete' || e.key === 'Backspace') && !typing && state.selected && onMap(state.selected) && !hidden.has(state.selected)) {
      e.preventDefault();
      hideFn(state.selected);
      return;
    }
    if (e.key === 'Escape') {
      if (!$('rxpanel').hidden) { togglePatternHelper(false); return; }
      if (!$('signpanel').hidden) { toggleSigns(false); return; }
      q.value = ''; state.query = ''; markSearch(); select(null, false);
    }
  });
}

function setDir(d) {
  state.dir = d;
  try { localStorage.setItem(DIR_KEY, d); } catch (e) {}
  $('o-h').classList.toggle('on', d === 'h');
  $('o-v').classList.toggle('on', d === 'v');
  drawAll(true);
  if (state.selected && main.placed[state.selected]) main.centerOn(state.selected);
}

function setMode(m) {
  state.mode = m;
  setFaces(m === 'all' && DATA.history);
  showHeader();
  main.expanded = new Set();
  wholeMapUnfolds = new Set();
  state.selected = null;
  state.focus = null;
  showOverview();
  $('m-changes').classList.toggle('on', m === 'changes');
  $('m-all').classList.toggle('on', m === 'all');
  main.draw(true);
}

// ------------------------------------------------------------ docking

/// Where the details go, and how big they are, are per-viewer preferences.
const DOCK_KEY = 'flowlens:dock';
const SIZE_KEY = 'flowlens:panel-size';
const store = {
  get: (k) => { try { return JSON.parse(localStorage.getItem(k)); } catch (e) { return null; } },
  set: (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch (e) {} },
};
let dock = 'right';
let popup = null;
/// Whether the details panel is open beside the map. Closed, what is
/// selected opens in a box over the map instead.
let panelOpen = true;

/// Show the box over the map only while there is something to show in it.
function syncBox() {
  const split = document.querySelector('.split');
  if (split) split.classList.toggle('modal-on', !!nav.cur && nav.cur.t !== 'overview');
}

/// The document the details live in: this page, or the separate window.
const panelDoc = () => (popup && !popup.closed ? popup.document : document);
const P = (id) => panelDoc().getElementById(id);

function renderPanel() {
  if (state.selected) showFunction(state.selected); else showOverview();
}

function setDock(d, remember = true) {
  if (d === 'window' && !openWindow()) {
    P('panel').insertAdjacentHTML('afterbegin', '<p class="popnote">Your browser blocked the new window. Allow pop-ups for this page and try again.</p>');
    return;
  }
  if (d !== 'window' && popup && !popup.closed) { const w = popup; popup = null; w.close(); }
  dock = d;
  const split = document.querySelector('.split');
  split.className = 'split dock-' + d + (panelOpen || d === 'window' ? '' : ' closed');
  syncBox();
  for (const x of ['right', 'left', 'bottom', 'window']) {
    const on = x === d && (panelOpen || d === 'window');
    $('d-' + x).classList.toggle('on', on);
    const where = { right: 'Details on the right', left: 'Details on the left', bottom: 'Details below the map' };
    if (x !== 'window') $('d-' + x).title = on ? 'Close the details — what you select opens in a box over the map' : where[x];
  }
  if (remember) store.set(DOCK_KEY, d);
  if (d !== 'window') renderPanel();
  // The map changed size; keep what was in view in view.
  requestAnimationFrame(() => {
    for (const m of [main, side]) if (m && !m.wrap.hidden) m.view.moved ? m.apply() : m.fit(false);
  });
}

/// Open the details in their own window, sharing this page's styles. The
/// window is driven from here, so clicks in either one update both.
function openWindow() {
  if (popup && !popup.closed) { popup.focus(); return true; }
  const w = window.open('', 'flowlens-details', 'width=560,height=860');
  if (!w) return false;
  const css = document.querySelector('style').textContent;
  w.document.open();
  const looks = Object.keys(LOOKS).map((what) => document.documentElement.dataset[what] ? ' data-' + what + '="' + esc(document.documentElement.dataset[what]) + '"' : '').join('');
  w.document.write('<!doctype html><html lang="en"' + looks + '><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">' +
    '<title>' + esc(DATA.crate) + ' — details</title><style>' + css + '</style></head><body class="popup"><aside class="panel" id="panel"></aside></body></html>');
  w.document.close();
  wireCuts(w.document);
  w.document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') select(null, false);
    if (e.altKey && e.key === 'ArrowLeft') { e.preventDefault(); goBack(); }
  });
  popup = w;
  renderPanel();
  // Closing the window puts the details back where they were.
  const back = store.get(DOCK_KEY + ':docked') || 'right';
  const watch = setInterval(() => {
    if (popup === w && w.closed) { clearInterval(watch); popup = null; setDock(back, false); }
    if (popup !== w) clearInterval(watch);
  }, 500);
  return true;
}

/// Clicking text cut short with "…" shows it whole, and clicking again
/// shortens it. Caught on the way down, so the click opens nothing else.
function wireCuts(doc) {
  const flip = (e) => {
    const c = e.target.closest && e.target.closest('.cut');
    if (!c) return;
    e.stopPropagation();
    e.preventDefault();
    const open = !c.classList.contains('open');
    c.classList.toggle('open', open);
    c.textContent = open ? c.dataset.full : c.dataset.short;
    c.title = open ? 'Click to shorten' : 'Click to show it all';
  };
  doc.addEventListener('click', flip, true);
  doc.addEventListener('keydown', (e) => { if (e.key === 'Enter') flip(e); }, true);
}

function wireDock() {
  for (const d of ['right', 'left', 'bottom', 'window']) {
    $('d-' + d).onclick = () => {
      if (d !== 'window') {
        // The side the panel is on, clicked again, closes it; any side
        // clicked while it is closed opens it there.
        panelOpen = !(panelOpen && dock === d);
        store.set(DOCK_KEY + ':open', panelOpen);
        store.set(DOCK_KEY + ':docked', d);
      }
      setDock(d);
    };
  }
  window.addEventListener('beforeunload', () => { if (popup) popup.close(); });

  // Drag the divider to resize the details.
  const split = document.querySelector('.split'), gutter = $('gutter');
  const size = store.get(SIZE_KEY) || {};
  if (size.w) split.style.setProperty('--panel-w', size.w + 'px');
  if (size.h) split.style.setProperty('--panel-h', size.h + 'px');
  let drag = false;
  gutter.addEventListener('pointerdown', (e) => { drag = true; gutter.setPointerCapture(e.pointerId); gutter.classList.add('drag'); });
  gutter.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const r = split.getBoundingClientRect();
    if (dock === 'bottom') {
      size.h = Math.round(Math.min(Math.max(r.bottom - e.clientY, 140), r.height * 0.8));
      split.style.setProperty('--panel-h', size.h + 'px');
    } else {
      const w = dock === 'left' ? e.clientX - r.left : r.right - e.clientX;
      size.w = Math.round(Math.min(Math.max(w, 280), r.width * 0.7));
      split.style.setProperty('--panel-w', size.w + 'px');
    }
  });
  gutter.addEventListener('pointerup', () => { drag = false; gutter.classList.remove('drag'); store.set(SIZE_KEY, size); });

  panelOpen = store.get(DOCK_KEY + ':open') !== false;
  const saved = store.get(DOCK_KEY);
  // A window cannot be reopened without a click, so start docked.
  const start = saved === 'window' ? store.get(DOCK_KEY + ':docked') || 'right' : saved || 'right';
  setDock(['right', 'left', 'bottom'].includes(start) ? start : 'right', false);
}

// ------------------------------------------------------------ start

/// The headline for each view: what changed, or the project as it is.
const header = { compared: '', changed: false };

/// "HEAD (7b36756 · Add workers · 2026-10-01)" → its parts, for a short
/// header line with the rest on hover.
const AGAINST = (() => {
  const m = /^(.*?) \(([0-9a-f]{4,}) · (.*) · ([^·]*)\)$/.exec(DATA.against || '');
  return m ? { ref: m[1], hash: m[2], msg: m[3], date: m[4] } : null;
})();
const againstShort = () => (AGAINST ? AGAINST.ref + ' · ' + AGAINST.hash : DATA.against);

function showHeader() {
  const head = $('headline'), against = $('against');
  const plain = PLAIN || !DATA.history;
  head.textContent = plain ? 'Current code' : header.compared;
  head.className = 'status' + (!plain && header.changed ? ' changed' : '');
  head.title = plain ? 'The code as it is now, compared with nothing' : 'Changed functions, and the features they are part of';
  if (!DATA.history) {
    against.textContent = 'not compared — ' + DATA.against;
    against.title = '';
  } else if (PLAIN) {
    against.textContent = '';
    against.title = '';
  } else {
    against.innerHTML = 'vs <span class="mono">' + esc(againstShort()) + '</span>';
    against.title = AGAINST ? 'Compared with ' + AGAINST.ref + ': commit ' + AGAINST.hash + ' “' + AGAINST.msg + '”, ' + AGAINST.date : 'Compared with ' + DATA.against;
  }
  document.querySelector('.legend').hidden = plain;
}

/// The Theme and Font menus: each sets `data-theme` / `data-font` on this
/// page and the details window, and is kept in this browser.
function wireLooks() {
  for (const [what, key] of Object.entries(LOOKS)) {
    const sel = $(what);
    sel.value = document.documentElement.dataset[what] || '';
    // A choice saved from a menu entry that is no longer offered: back to
    // the default.
    if (sel.selectedIndex < 0) {
      sel.value = '';
      delete document.documentElement.dataset[what];
      try { localStorage.removeItem(key); } catch (e) {}
    }
    sel.onchange = () => {
      const v = sel.value;
      for (const doc of [document, popup && !popup.closed ? popup.document : null]) {
        if (!doc) continue;
        if (v) doc.documentElement.dataset[what] = v; else delete doc.documentElement.dataset[what];
      }
      try { v ? localStorage.setItem(key, v) : localStorage.removeItem(key); } catch (e) {}
      // Labels on the map are cut to fit the font they are drawn in.
      if (what === 'font') { main.draw(false); if (side) side.draw(false); }
    };
  }
}

function start() {
  wireLooks();
  const touched = DATA.features.filter((f) => f.s !== 'same').length;
  const changed = KEYS.filter((k) => onMap(k) && F[k].s !== 'same').length;
  $('crate').textContent = DATA.crate;
  header.compared = changed ? plural(changed, 'change', 'changes') + ' · ' + plural(touched, 'feature', 'features') : 'No changes';
  header.changed = changed > 0;
  if (!DATA.history) {
    // Nothing to compare with, so there are no changes to show — say why
    // rather than hiding the option.
    $('m-changes').disabled = true;
    $('m-changes').title = 'Needs git history: commit the project once, make a change, and run this again';
    document.querySelector('.legend').hidden = true;
  }
  wireCuts(document);
  $('m-changes').onclick = () => setMode('changes');
  $('m-all').onclick = () => setMode('all');
  $('o-h').onclick = () => setDir('h');
  $('o-v').onclick = () => setDir('v');
  $('o-h').classList.toggle('on', state.dir === 'h');
  $('o-v').classList.toggle('on', state.dir === 'v');
  main = new MapView($('mapwrap'), (expanded) => visible(expanded));
  wireHidden();
  side = new MapView($('sidewrap'), (expanded) => (flowShown() ? asideView(state.asideOn, expanded) : { nodes: [], edges: [] }), { side: true });
  $('structbtn').onclick = () => (state.structureOpen && state.sideTab === 'structure' && !$('sidewrap').hidden ? closeStructure() : showStructure());
  wireSearch();
  wireDock();
  wireMapsGutter();
  if (!changed) { state.mode = 'all'; $('m-changes').classList.remove('on'); $('m-all').classList.add('on'); }
  setFaces(state.mode === 'all' && DATA.history);
  showHeader();
  main.draw(true);
  const fromUrl = decodeURIComponent(location.hash.slice(1));
  if (fromUrl && F[fromUrl]) select(fromUrl, true); else showOverview();
}

start();
"##;
