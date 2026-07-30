use crate::doc_builder::{Document, ListKind, Node};
use crate::renderers::Renderer;

#[derive(Debug, Default, Clone, Copy)]
pub struct MarkdownRenderer;

impl Renderer for MarkdownRenderer {
    fn extension(&self) -> &'static str {
        "md"
    }

    fn render(&self, doc: &Document) -> String {
        render(doc)
    }
}

pub fn render(doc: &Document) -> String {
    let mut out = String::with_capacity(doc.nodes().len() * 64);

    for node in doc.nodes() {
        render_node(&mut out, node);
        // Blocks are separated by a blank line. Inline nodes at top level run together.
        if node.is_block() {
            end_block(&mut out);
        }
    }

    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

fn end_block(out: &mut String) {
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
}

fn render_node(out: &mut String, node: &Node) {
    match node {
        Node::Text(text) => out.push_str(text),
        Node::Bold { children } => {
            out.push_str("**");
            render_all(out, children);
            out.push_str("**");
        }
        Node::Italic { children } => {
            out.push('*');
            render_all(out, children);
            out.push('*');
        }
        Node::InlineCode(code) => {
            // A span containing a backtick needs a longer fence than the run it holds.
            let fence = "`".repeat(longest_backtick_run(code) + 1);
            out.push_str(&fence);
            out.push_str(code);
            out.push_str(&fence);
        }
        Node::Link { href, children } => {
            out.push('[');
            render_all(out, children);
            out.push_str("](");
            out.push_str(href);
            out.push(')');
        }
        Node::Heading { level, children } => {
            for _ in 0..level.level() {
                out.push('#');
            }
            out.push(' ');
            render_all(out, children);
        }
        Node::Paragraph { children } => render_all(out, children),
        Node::CodeBlock { language, content } => {
            let fence = "`".repeat(longest_backtick_run(content).max(2) + 1);
            out.push_str(&fence);
            if let Some(lang) = language {
                out.push_str(lang);
            }
            out.push('\n');
            out.push_str(content);
            if !content.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&fence);
        }
        Node::List { kind, items } => {
            for (i, item) in items.iter().enumerate() {
                match kind {
                    ListKind::Bullet => out.push_str("- "),
                    ListKind::Numbered => {
                        out.push_str(&(i + 1).to_string());
                        out.push_str(". ");
                    }
                }
                render_all(out, item);
                out.push('\n');
            }
        }
        Node::Rule => out.push_str("---"),
        Node::Raw(raw) => out.push_str(raw),
    }
}

fn render_all(out: &mut String, nodes: &[Node]) {
    for node in nodes {
        render_node(out, node);
    }
}

/// Longest consecutive backtick run in `text`, so a fence can always out-length it.
fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;

    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }

    longest
}
