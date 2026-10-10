//! Control-flow graph per function — the original experiment, kept intact.
//! Emits one Graphviz cluster per `fn`, with branch/loop/match structure.

use ra_ap_syntax::{SourceFile, SyntaxKind, SyntaxNode};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone)]
enum FlowNode {
    Start(String),        // Function start
    End(String),          // Function end
    VarDecl(String),      // Variable declaration: let x = ...
    Assignment(String),   // Assignment: x = ...
    FunctionCall(String), // Function call: foo(args)
    MethodCall(String),   // Method call: obj.method(args)
    IfCondition(String),  // If condition
    ElseBranch,           // Else branch
    LoopStart(String),    // Loop (for, while)
    LoopEnd,              // End of loop
    Return(String),       // Return statement
    Break,                // Break statement
    Continue,             // Continue statement
    Match(String),        // Match expression
    MatchArm(String),     // Match arm pattern
    Merge,                // Merge node for CFG continuation
}

impl FlowNode {
    fn to_label(&self) -> String {
        match self {
            FlowNode::Start(name) => format!("START: {}", name),
            FlowNode::End(name) => format!("END: {}", name),
            FlowNode::VarDecl(code) => format!("VAR: {}", code),
            FlowNode::Assignment(code) => format!("ASSIGN: {}", code),
            FlowNode::FunctionCall(code) => format!("CALL: {}", code),
            FlowNode::MethodCall(code) => format!("METHOD: {}", code),
            FlowNode::IfCondition(cond) => format!("IF: {}", cond),
            FlowNode::ElseBranch => "ELSE".to_string(),
            FlowNode::LoopStart(kind) => format!("LOOP: {}", kind),
            FlowNode::LoopEnd => "END LOOP".to_string(),
            FlowNode::Return(val) => format!("RETURN: {}", val),
            FlowNode::Break => "BREAK".to_string(),
            FlowNode::Continue => "CONTINUE".to_string(),
            FlowNode::Match(expr) => format!("MATCH: {}", expr),
            FlowNode::MatchArm(pattern) => format!("ARM: {}", pattern),
            FlowNode::Merge => "MERGE".to_string(),
        }
    }

    fn get_color(&self) -> &str {
        match self {
            FlowNode::Start(_) => "#90EE90",                   // Light green
            FlowNode::End(_) => "#FFB6C1",                     // Light pink
            FlowNode::VarDecl(_) => "#87CEEB",                 // Sky blue
            FlowNode::Assignment(_) => "#B0E0E6",              // Powder blue
            FlowNode::FunctionCall(_) => "#FFD700",            // Gold
            FlowNode::MethodCall(_) => "#FFA500",              // Orange
            FlowNode::IfCondition(_) => "#FF6B6B",             // Red
            FlowNode::ElseBranch => "#FF8C8C",                 // Light red
            FlowNode::LoopStart(_) => "#9370DB",               // Purple
            FlowNode::LoopEnd => "#BA9BDB",                    // Light purple
            FlowNode::Return(_) => "#F08080",                  // Light coral
            FlowNode::Break | FlowNode::Continue => "#DDA0DD", // Plum
            FlowNode::Match(_) => "#20B2AA",                   // Light sea green
            FlowNode::MatchArm(_) => "#48D1CC",                // Medium turquoise
            FlowNode::Merge => "#D3D3D3",                      // Light gray
        }
    }

    fn get_shape(&self) -> &str {
        match self {
            FlowNode::Start(_) | FlowNode::End(_) => "ellipse",
            FlowNode::IfCondition(_) | FlowNode::Match(_) => "diamond",
            FlowNode::LoopStart(_) => "hexagon",
            FlowNode::Merge => "circle",
            _ => "box",
        }
    }
}

struct FlowGraph {
    nodes: Vec<(usize, FlowNode)>,              // (id, node)
    edges: Vec<(usize, usize, Option<String>)>, // (from, to, label)
    next_id: usize,
}

impl FlowGraph {
    fn new() -> Self {
        FlowGraph { nodes: Vec::new(), edges: Vec::new(), next_id: 0 }
    }

    fn add_node(&mut self, node: FlowNode) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push((id, node));
        id
    }

    fn add_edge(&mut self, from: usize, to: usize, label: Option<String>) {
        self.edges.push((from, to, label));
    }
}

fn escape_html(s: &str) -> String {
    s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;")
}

/// Process a statement, always returns possible exit nodes
fn process_statement(stmt: &SyntaxNode, graph: &mut FlowGraph, ignore_list: &HashSet<&str>, prev_node: usize) -> Option<Vec<usize>> {
    match stmt.kind() {
        SyntaxKind::LET_STMT => {
            let text = stmt.text().to_string().trim().to_string();
            let node_id = graph.add_node(FlowNode::VarDecl(text));
            graph.add_edge(prev_node, node_id, None);
            Some(vec![node_id])
        }
        SyntaxKind::EXPR_STMT => stmt.first_child().map(|expr| process_expression(&expr, graph, ignore_list, prev_node)).or(Some(vec![prev_node])),
        SyntaxKind::RETURN_EXPR => {
            let val = stmt.text().to_string();
            let node_id = graph.add_node(FlowNode::Return(val));
            graph.add_edge(prev_node, node_id, None);
            Some(vec![node_id])
        }
        _ => Some(vec![prev_node]),
    }
}

/// Process an expression, always returns possible exit nodes
fn process_expression(expr: &SyntaxNode, graph: &mut FlowGraph, ignore_list: &HashSet<&str>, prev_node: usize) -> Vec<usize> {
    match expr.kind() {
        SyntaxKind::CALL_EXPR => {
            if let Some(callee) = expr.children().next() {
                if callee.kind() == SyntaxKind::PATH_EXPR {
                    let name = callee.text().to_string();
                    if ignore_list.contains(name.as_str()) {
                        return vec![prev_node];
                    }
                }
            }
            vec![graph.add_node(FlowNode::FunctionCall(expr.text().to_string()))]
        }
        SyntaxKind::METHOD_CALL_EXPR => {
            if let Some(name_ref) = expr.children().find(|n| n.kind() == SyntaxKind::NAME_REF) {
                let method_name = name_ref.text().to_string();
                if ignore_list.contains(method_name.as_str()) {
                    return vec![prev_node];
                }
            }
            vec![graph.add_node(FlowNode::MethodCall(expr.text().to_string()))]
        }
        SyntaxKind::IF_EXPR => process_if_expression(expr, graph, ignore_list, prev_node),
        SyntaxKind::LOOP_EXPR | SyntaxKind::WHILE_EXPR | SyntaxKind::FOR_EXPR => process_loop_expression(expr, graph, ignore_list, prev_node),
        SyntaxKind::MATCH_EXPR => process_match_expression(expr, graph, ignore_list, prev_node),
        SyntaxKind::BREAK_EXPR => vec![graph.add_node(FlowNode::Break)],
        SyntaxKind::CONTINUE_EXPR => vec![graph.add_node(FlowNode::Continue)],
        SyntaxKind::BIN_EXPR => {
            let text = expr.text().to_string();
            if text.contains('=') && !text.contains("==") { vec![graph.add_node(FlowNode::Assignment(text))] } else { vec![prev_node] }
        }
        _ => vec![prev_node],
    }
}

fn process_if_expression(if_expr: &SyntaxNode, graph: &mut FlowGraph, ignore_list: &HashSet<&str>, prev_node: usize) -> Vec<usize> {
    let condition = if_expr.children().find(|n| !matches!(n.kind(), SyntaxKind::IF_KW | SyntaxKind::BLOCK_EXPR | SyntaxKind::ELSE_KW)).map(|n| n.text().to_string()).unwrap_or_else(|| "?".to_string());

    let if_node = graph.add_node(FlowNode::IfCondition(condition));
    graph.add_edge(prev_node, if_node, None);

    let mut exits = Vec::new();
    let mut block_queue: VecDeque<SyntaxNode> = if_expr.children().filter(|n| n.kind() == SyntaxKind::BLOCK_EXPR).collect();

    // --- THEN branch ---
    if let Some(then_block) = block_queue.pop_front() {
        if let Some((first_then, last_then)) = process_block(&then_block, graph, ignore_list, if_node) {
            graph.add_edge(if_node, first_then, Some("true".into()));
            exits.extend(last_then);
        }
    }

    // --- ELSE branch ---
    if let Some(else_kw) = if_expr.children().find(|n| n.kind() == SyntaxKind::ELSE_KW) {
        if let Some(else_target) = else_kw.next_sibling() {
            let else_node = graph.add_node(FlowNode::ElseBranch);
            graph.add_edge(if_node, else_node, Some("false".into()));

            let else_exits = match else_target.kind() {
                SyntaxKind::BLOCK_EXPR => {
                    if let Some((first_else, last_else)) = process_block(&else_target, graph, ignore_list, else_node) {
                        graph.add_edge(else_node, first_else, None);
                        last_else
                    } else {
                        vec![else_node]
                    }
                }
                SyntaxKind::IF_EXPR => process_if_expression(&else_target, graph, ignore_list, else_node),
                _ => process_statement(&else_target, graph, ignore_list, else_node).unwrap_or_else(|| vec![else_node]),
            };

            exits.extend(else_exits);
        }
    } else if let Some(else_block) = block_queue.pop_front() {
        // Fallback if parser gives a block without ELSE_KW
        let else_node = graph.add_node(FlowNode::ElseBranch);
        graph.add_edge(if_node, else_node, Some("false".into()));
        if let Some((first_else, last_else)) = process_block(&else_block, graph, ignore_list, else_node) {
            graph.add_edge(else_node, first_else, None);
            exits.extend(last_else);
        } else {
            exits.push(else_node);
        }
    } else {
        // No else
        exits.push(if_node);
    }

    exits
}

fn process_loop_expression(loop_expr: &SyntaxNode, graph: &mut FlowGraph, ignore_list: &HashSet<&str>, prev_node: usize) -> Vec<usize> {
    let loop_kind = match loop_expr.kind() {
        SyntaxKind::LOOP_EXPR => "loop",
        SyntaxKind::WHILE_EXPR => "while",
        SyntaxKind::FOR_EXPR => "for",
        _ => "loop",
    };

    let condition = if loop_expr.kind() == SyntaxKind::WHILE_EXPR {
        loop_expr.children().find(|n| n.kind() != SyntaxKind::WHILE_KW && n.kind() != SyntaxKind::BLOCK_EXPR).and_then(|c| c.first_child()).map(|e| format!("{} {}", loop_kind, e.text())).unwrap_or_else(|| loop_kind.to_string())
    } else if loop_expr.kind() == SyntaxKind::FOR_EXPR {
        format!("{} ...", loop_kind)
    } else {
        loop_kind.to_string()
    };

    let loop_start = graph.add_node(FlowNode::LoopStart(condition));
    graph.add_edge(prev_node, loop_start, None);

    if let Some(body) = loop_expr.children().find(|n| n.kind() == SyntaxKind::BLOCK_EXPR) {
        if let Some((first_node, body_last)) = process_block(&body, graph, ignore_list, loop_start) {
            graph.add_edge(loop_start, first_node, Some("loop".to_string()));
            for last in body_last {
                graph.add_edge(last, loop_start, Some("repeat".to_string()));
            }
        }
    }

    // Loop exit node
    let loop_end = graph.add_node(FlowNode::LoopEnd);
    graph.add_edge(loop_start, loop_end, Some("exit".to_string()));

    vec![loop_end]
}

fn process_match_expression(match_expr: &SyntaxNode, graph: &mut FlowGraph, ignore_list: &HashSet<&str>, prev_node: usize) -> Vec<usize> {
    let expr_text = match_expr.children().find(|n| n.kind() != SyntaxKind::MATCH_KW && n.kind() != SyntaxKind::MATCH_ARM_LIST).map(|e| e.text().to_string()).unwrap_or_else(|| "?".to_string());

    let match_node = graph.add_node(FlowNode::Match(expr_text));
    graph.add_edge(prev_node, match_node, None);

    let mut last_nodes = Vec::new();

    if let Some(arm_list) = match_expr.children().find(|n| n.kind() == SyntaxKind::MATCH_ARM_LIST) {
        for arm in arm_list.children().filter(|n| n.kind() == SyntaxKind::MATCH_ARM) {
            let pattern = arm.children().find(|n| matches!(n.kind(), SyntaxKind::WILDCARD_PAT | SyntaxKind::LITERAL_PAT | SyntaxKind::IDENT_PAT)).map(|p| p.text().to_string()).unwrap_or_else(|| "_".to_string());

            let arm_node = graph.add_node(FlowNode::MatchArm(pattern));
            graph.add_edge(match_node, arm_node, None);

            if let Some(body) = arm.children().find(|n| n.kind() == SyntaxKind::BLOCK_EXPR) {
                if let Some((first_node, body_last)) = process_block(&body, graph, ignore_list, arm_node) {
                    graph.add_edge(arm_node, first_node, None);
                    last_nodes.extend(body_last);
                } else {
                    last_nodes.push(arm_node);
                }
            } else {
                last_nodes.push(arm_node);
            }
        }
    }

    // Merge node for continuation
    let merge_node = graph.add_node(FlowNode::Merge);
    for node in &last_nodes {
        graph.add_edge(*node, merge_node, None);
    }

    vec![merge_node]
}

/// Process a block and return (first_node, exit_nodes)
fn process_block(block: &SyntaxNode, graph: &mut FlowGraph, ignore_list: &HashSet<&str>, start_id: usize) -> Option<(usize, Vec<usize>)> {
    let mut first_node = None;
    let mut prev_nodes = vec![start_id];

    // Prefer STMT_LIST if it exists
    if let Some(stmt_list) = block.children().find(|n| n.kind() == SyntaxKind::STMT_LIST) {
        for stmt in stmt_list.children() {
            if stmt.kind().is_trivia() {
                continue;
            }

            let mut new_prev_nodes = vec![];
            for prev in prev_nodes {
                let ids = process_statement(&stmt, graph, ignore_list, prev).unwrap_or_else(|| vec![prev]);
                new_prev_nodes.extend(ids);
            }

            prev_nodes = new_prev_nodes;
            if first_node.is_none() && !prev_nodes.is_empty() {
                first_node = Some(prev_nodes[0]);
            }
        }
    } else {
        // Handle single expression directly inside block (no STMT_LIST)
        if let Some(expr) = block.children().find(|n| n.kind() != SyntaxKind::L_CURLY && n.kind() != SyntaxKind::R_CURLY) {
            let mut new_prev_nodes = vec![];
            for prev in prev_nodes {
                let ids = process_expression(&expr, graph, ignore_list, prev);
                new_prev_nodes.extend(ids);
            }

            prev_nodes = new_prev_nodes;
            if first_node.is_none() && !prev_nodes.is_empty() {
                first_node = Some(prev_nodes[0]);
            }
        }
    }

    first_node.map(|f| (f, prev_nodes))
}

/// Emit a control-flow DOT graph for every function in `src`.
/// Names in `ignore` are skipped as call nodes (e.g. `println`).
pub fn emit_cfg_dot(src: &str, ignore: &[&str]) -> String {
    let ignore_list: HashSet<&str> = ignore.iter().copied().collect();
    let mut buf = String::new();

    let parsed = SourceFile::parse(src, ra_ap_syntax::Edition::Edition2021);
    let root = parsed.syntax_node();

    // let syntax: SyntaxNode = parsed.syntax_node();

    // out(&mut buf, format!("{:#?}", syntax));

    // Generate DOT output
    out(&mut buf, "digraph control_flow {".to_string());
    out(&mut buf, "    rankdir=TB;".to_string());
    out(&mut buf, "    ranksep=1.0;".to_string());
    out(&mut buf, "    nodesep=0.8;".to_string());
    out(&mut buf, "    node [style=filled, fontname=\"Arial\", margin=\"0.2,0.2\"];".to_string());
    out(&mut buf, "    edge [fontname=\"Arial\", fontsize=10];".to_string());
    out(&mut buf, "    compound=true;".to_string());
    out(&mut buf, "    ".to_string());

    // Process each function
    for (idx, func) in root.descendants().filter(|n| n.kind() == SyntaxKind::FN).enumerate() {
        if let Some(name_node) = func.children().find(|n| n.kind() == SyntaxKind::NAME) {
            let label = name_node.text().to_string();
            // Two impls can each define `new`; the label may repeat but the
            // DOT identifier must not, or their graphs merge into one.
            let func_name = format!("{label}_{idx}");
            let mut graph = FlowGraph::new();

            // Add start node
            let start_id = graph.add_node(FlowNode::Start(label.clone()));

            // Process function body
            if let Some(body) = func.children().find(|n| n.kind() == SyntaxKind::BLOCK_EXPR) {
                if let Some((first_node, exit_nodes)) = process_block(&body, &mut graph, &ignore_list, start_id) {
                    // Attach start node to first node of the body
                    graph.add_edge(start_id, first_node, None);

                    // Add end node
                    let end_id = graph.add_node(FlowNode::End(label.clone()));
                    for last_node in exit_nodes {
                        graph.add_edge(last_node, end_id, None);
                    }
                }
            }

            // Output subgraph for this function
            out(&mut buf, format!("    subgraph cluster_{} {{", func_name));
            out(&mut buf, format!("        label=\"Function: {}\";", label));
            out(&mut buf, "        style=rounded;".to_string());
            out(&mut buf, "        color=blue;".to_string());
            out(&mut buf, "        ".to_string());

            // Output nodes
            for (id, node) in &graph.nodes {
                let label = escape_html(&node.to_label());
                let color = node.get_color();
                let shape = node.get_shape();
                out(&mut buf, format!("        node_{}_{} [label=\"{}\", fillcolor=\"{}\", shape={}];", func_name, id, label, color, shape));
            }
            out(&mut buf, "        ".to_string());

            // Output edges with proper constraints
            for (from, to, label) in &graph.edges {
                if let Some(lbl) = label {
                    out(&mut buf, format!("        node_{}_{} -> node_{}_{} [label=\"{}\", color=blue];", func_name, from, func_name, to, lbl));
                } else {
                    out(&mut buf, format!("        node_{}_{} -> node_{}_{} [color=black];", func_name, from, func_name, to));
                }
            }

            out(&mut buf, "    }".to_string());
            out(&mut buf, "    ".to_string());
        }
    }

    out(&mut buf, "}".to_string());

    buf
}

fn out(buf: &mut String, line: String) {
    buf.push_str(&line);
    buf.push('\n');
}
