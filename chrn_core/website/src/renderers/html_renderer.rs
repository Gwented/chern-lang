use crate::doc_builder::{Document, ListKind, Node};
use crate::renderers::Renderer;

/// Renders a document as HTML. Default is a bare fragment; [`HtmlRenderer::page`] wraps it in a
/// standalone document with `<head>`.
#[derive(Debug, Default, Clone)]
pub struct HtmlRenderer {
    page: Option<PageShell>,
}

#[derive(Debug, Default, Clone)]
struct PageShell {
    title: String,
    /// Href of a stylesheet, relative to the emitted file.
    stylesheet: Option<String>,
    lang: Option<String>,
}

impl HtmlRenderer {
    /// Fragment output — no `<html>`, `<head>`, or `<body>`.
    pub const fn fragment() -> Self {
        Self { page: None }
    }

    /// Standalone page output with `title` in the `<head>`.
    pub fn page<S: Into<String>>(title: S) -> Self {
        Self {
            page: Some(PageShell {
                title: title.into(),
                stylesheet: None,
                lang: Some("en".into()),
            }),
        }
    }

    /// Link a stylesheet. Ignored in fragment mode.
    pub fn with_stylesheet<S: Into<String>>(mut self, href: S) -> Self {
        if let Some(shell) = self.page.as_mut() {
            shell.stylesheet = Some(href.into());
        }
        self
    }

    /// Override the `<html lang=..>`. Ignored in fragment mode.
    pub fn with_lang<S: Into<String>>(mut self, lang: S) -> Self {
        if let Some(shell) = self.page.as_mut() {
            shell.lang = Some(lang.into());
        }
        self
    }
}

impl Renderer for HtmlRenderer {
    fn extension(&self) -> &'static str {
        "html"
    }

    fn render(&self, doc: &Document) -> String {
        let body = render_fragment(doc);

        let Some(shell) = &self.page else {
            return body;
        };

        let mut out = String::with_capacity(body.len() + 256);
        out.push_str("<!DOCTYPE html>\n<html");
        if let Some(lang) = &shell.lang {
            out.push_str(" lang=\"");
            push_escaped_attr(&mut out, lang);
            out.push('"');
        }
        out.push_str(">\n<head>\n<meta charset=\"utf-8\">\n");
        out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        out.push_str("<title>");
        push_escaped_text(&mut out, &shell.title);
        out.push_str("</title>\n");
        if let Some(href) = &shell.stylesheet {
            out.push_str("<link rel=\"stylesheet\" href=\"");
            push_escaped_attr(&mut out, href);
            out.push_str("\">\n");
        }
        out.push_str("</head>\n<body>\n");
        out.push_str(&body);
        out.push_str("</body>\n</html>\n");
        out
    }
}

pub fn render_fragment(doc: &Document) -> String {
    let mut out = String::with_capacity(doc.nodes().len() * 64);

    for node in doc.nodes() {
        render_node(&mut out, node);
        if node.is_block() {
            out.push('\n');
        }
    }

    out
}

fn render_node(out: &mut String, node: &Node) {
    match node {
        Node::Text(text) => push_escaped_text(out, text),
        Node::Bold { children } => wrap(out, "strong", children),
        Node::Italic { children } => wrap(out, "em", children),
        Node::InlineCode(code) => {
            out.push_str("<code>");
            push_escaped_text(out, code);
            out.push_str("</code>");
        }
        Node::Link { href, children } => {
            out.push_str("<a href=\"");
            push_escaped_attr(out, href);
            out.push_str("\">");
            render_all(out, children);
            out.push_str("</a>");
        }
        Node::Image { src, alt, title } => {
            out.push_str("<img src=\"");
            push_escaped_attr(out, src);
            out.push_str("\" alt=\"");
            push_escaped_attr(out, alt);
            out.push('"');
            if let Some(title) = title {
                out.push_str(" title=\"");
                push_escaped_attr(out, title);
                out.push('"');
            }
            out.push('>');
        }
        Node::Video(video) => {
            out.push_str("<video src=\"");
            push_escaped_attr(out, &video.src);
            out.push('"');
            if let Some(poster) = &video.poster {
                out.push_str(" poster=\"");
                push_escaped_attr(out, poster);
                out.push('"');
            }
            for (flag, name) in [
                (video.controls, "controls"),
                (video.autoplay, "autoplay"),
                (video.loops, "loop"),
                (video.muted, "muted"),
            ] {
                if flag {
                    out.push(' ');
                    out.push_str(name);
                }
            }
            // Autoplay on mobile needs it, and it is inert elsewhere.
            if video.autoplay {
                out.push_str(" playsinline");
            }
            // Shown only where <video> is unsupported.
            out.push_str(">\n<a href=\"");
            push_escaped_attr(out, &video.src);
            out.push_str("\">");
            push_escaped_text(out, &video.label);
            out.push_str("</a>\n</video>");
        }
        Node::Heading { level, children } => {
            let tag = format!("h{}", level.level());
            wrap(out, &tag, children);
        }
        Node::Paragraph { children } => wrap(out, "p", children),
        Node::CodeBlock { language, content } => {
            out.push_str("<pre><code");
            if let Some(lang) = language {
                out.push_str(" class=\"language-");
                push_escaped_attr(out, lang);
                out.push('"');
            }
            out.push('>');
            push_escaped_text(out, content);
            out.push_str("</code></pre>");
        }
        Node::List { kind, items } => {
            let tag = match kind {
                ListKind::Bullet => "ul",
                ListKind::Numbered => "ol",
            };
            out.push('<');
            out.push_str(tag);
            out.push_str(">\n");
            for item in items {
                out.push_str("<li>");
                render_all(out, item);
                out.push_str("</li>\n");
            }
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
        Node::Rule => out.push_str("<hr>"),
        Node::Raw(raw) => out.push_str(raw),
    }
}

fn wrap(out: &mut String, tag: &str, children: &[Node]) {
    out.push('<');
    out.push_str(tag);
    out.push('>');
    render_all(out, children);
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn render_all(out: &mut String, nodes: &[Node]) {
    for node in nodes {
        render_node(out, node);
    }
}

fn push_escaped_text(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn push_escaped_attr(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
}
