//! Backend document abstraction. Knows nothing about `chrn`, error codes, or the site layout.
//!
//! A [`Document`] is a list of [`Node`]s. Builders produce nodes; renderers consume them. Adding a
//! variant means handling it in every renderer — the matches are exhaustive on purpose.

use crate::renderers::Renderer;

/// Highest heading a document can express.
pub const MAX_HEADING_LEVEL: u8 = 6;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Document {
    pub nodes: Vec<Node>,
}

impl Document {
    pub const fn new(nodes: Vec<Node>) -> Self {
        Self { nodes }
    }

    pub fn builder() -> DocumentBuilder {
        DocumentBuilder::default()
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Render with any backend.
    pub fn render<R: Renderer>(&self, renderer: &R) -> String {
        renderer.render(self)
    }
}

/// Heading depth, clamped to `1..=`[`MAX_HEADING_LEVEL`] at construction so no renderer has to
/// defend against an `<h0>` or an `<h9000>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeadingLevel {
    level: u8,
}

impl HeadingLevel {
    pub const fn new(level: u8) -> Self {
        if level < 1 {
            Self { level: 1 }
        } else if level > MAX_HEADING_LEVEL {
            Self {
                level: MAX_HEADING_LEVEL,
            }
        } else {
            Self { level }
        }
    }

    pub const fn level(self) -> u8 {
        self.level
    }
}

impl From<u8> for HeadingLevel {
    fn from(level: u8) -> Self {
        Self::new(level)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Numbered,
}

/// A video and how it plays. `controls` defaults on; every other flag defaults off.
///
/// Markdown has no video element, so the markdown renderer degrades it to a link to [`Video::src`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    /// Href of the file, relative to the emitted page.
    pub src: String,
    /// Fallback text. Also the link text in markdown.
    pub label: String,
    /// Still shown before playback.
    pub poster: Option<String>,
    pub controls: bool,
    pub autoplay: bool,
    pub loops: bool,
    pub muted: bool,
}

impl Video {
    pub fn new<S: Into<String>, L: Into<String>>(src: S, label: L) -> Self {
        Self {
            src: src.into(),
            label: label.into(),
            poster: None,
            controls: true,
            autoplay: false,
            loops: false,
            muted: false,
        }
    }

    pub fn with_poster<S: Into<String>>(mut self, poster: S) -> Self {
        self.poster = Some(poster.into());
        self
    }

    pub const fn with_controls(mut self, controls: bool) -> Self {
        self.controls = controls;
        self
    }

    /// Autoplay, muted, looping, no controls — a silent demo clip.
    pub const fn looping_clip(mut self) -> Self {
        self.autoplay = true;
        self.loops = true;
        self.muted = true;
        self.controls = false;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    // -- Inline --
    Text(String),
    Bold {
        children: Vec<Node>,
    },
    Italic {
        children: Vec<Node>,
    },
    /// Single-line code span, not a block.
    InlineCode(String),
    Link {
        href: String,
        children: Vec<Node>,
    },
    /// `alt` is required — a decorative image passes an empty string deliberately.
    Image {
        src: String,
        alt: String,
        /// Hover text, not a caption.
        title: Option<String>,
    },

    // -- Block --
    Heading {
        level: HeadingLevel,
        children: Vec<Node>,
    },
    Paragraph {
        children: Vec<Node>,
    },
    CodeBlock {
        language: Option<String>,
        content: String,
    },
    List {
        kind: ListKind,
        items: Vec<Vec<Node>>,
    },
    /// Horizontal rule / thematic break.
    Rule,
    /// Block-level video. See [`Video`].
    Video(Video),
    /// Escape hatch. Emitted verbatim and unescaped by every renderer, so the caller owns its
    /// correctness for whichever backend runs.
    Raw(String),
}

impl Node {
    /// Whether this node stands alone in the block flow. Renderers use it to decide separation.
    pub const fn is_block(&self) -> bool {
        matches!(
            self,
            Node::Heading { .. }
                | Node::Paragraph { .. }
                | Node::CodeBlock { .. }
                | Node::List { .. }
                | Node::Rule
                | Node::Video(_)
                | Node::Raw(_)
        )
    }
}

/// A run of inline nodes. Exists so callers never write `Node::Text(..)` by hand.
///
/// `&str` and `String` convert into a single text node, so `.heading(2, "Title")` and
/// `.heading(2, Inline::new().text("see ").code("i32"))` are both valid.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Inline {
    nodes: Vec<Node>,
}

impl Inline {
    pub const fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn text<S: Into<String>>(mut self, text: S) -> Self {
        self.nodes.push(Node::Text(text.into()));
        self
    }

    pub fn bold<I: Into<Inline>>(mut self, content: I) -> Self {
        self.nodes.push(Node::Bold {
            children: content.into().into_nodes(),
        });
        self
    }

    pub fn italic<I: Into<Inline>>(mut self, content: I) -> Self {
        self.nodes.push(Node::Italic {
            children: content.into().into_nodes(),
        });
        self
    }

    pub fn code<S: Into<String>>(mut self, code: S) -> Self {
        self.nodes.push(Node::InlineCode(code.into()));
        self
    }

    pub fn link<S: Into<String>, I: Into<Inline>>(mut self, href: S, content: I) -> Self {
        self.nodes.push(Node::Link {
            href: href.into(),
            children: content.into().into_nodes(),
        });
        self
    }

    /// Inline image. `alt` is what a reader gets when the file does not load.
    pub fn image<S: Into<String>, A: Into<String>>(mut self, src: S, alt: A) -> Self {
        self.nodes.push(Node::Image {
            src: src.into(),
            alt: alt.into(),
            title: None,
        });
        self
    }

    /// Inline image with hover text.
    pub fn image_titled<S: Into<String>, A: Into<String>, T: Into<String>>(
        mut self,
        src: S,
        alt: A,
        title: T,
    ) -> Self {
        self.nodes.push(Node::Image {
            src: src.into(),
            alt: alt.into(),
            title: Some(title.into()),
        });
        self
    }

    pub fn raw<S: Into<String>>(mut self, raw: S) -> Self {
        self.nodes.push(Node::Raw(raw.into()));
        self
    }

    /// Append another run, for when a caller assembles a fragment separately.
    pub fn join(mut self, other: Inline) -> Self {
        self.nodes.extend(other.nodes);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn into_nodes(self) -> Vec<Node> {
        self.nodes
    }
}

impl From<&str> for Inline {
    fn from(text: &str) -> Self {
        Inline::new().text(text)
    }
}

impl From<String> for Inline {
    fn from(text: String) -> Self {
        Inline::new().text(text)
    }
}

impl From<&String> for Inline {
    fn from(text: &String) -> Self {
        Inline::new().text(text.as_str())
    }
}

impl From<Node> for Inline {
    fn from(node: Node) -> Self {
        Self { nodes: vec![node] }
    }
}

impl From<Vec<Node>> for Inline {
    fn from(nodes: Vec<Node>) -> Self {
        Self { nodes }
    }
}

// Markdown markdown file
/// General-purpose instruction handler for emitting markdown formats like html and markdown
#[derive(Debug, Default, Clone)]
pub struct DocumentBuilder {
    nodes: Vec<Node>,
}

impl DocumentBuilder {
    pub const fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn heading<L: Into<HeadingLevel>, I: Into<Inline>>(mut self, level: L, content: I) -> Self {
        self.nodes.push(Node::Heading {
            level: level.into(),
            children: content.into().into_nodes(),
        });
        self
    }

    pub fn paragraph<I: Into<Inline>>(mut self, content: I) -> Self {
        self.nodes.push(Node::Paragraph {
            children: content.into().into_nodes(),
        });
        self
    }

    pub fn code_block<S: Into<String>>(mut self, language: Option<String>, content: S) -> Self {
        self.nodes.push(Node::CodeBlock {
            language,
            content: content.into(),
        });
        self
    }

    pub fn bullets<I, C>(self, items: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Inline>,
    {
        self.list(ListKind::Bullet, items)
    }

    pub fn numbered<I, C>(self, items: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Inline>,
    {
        self.list(ListKind::Numbered, items)
    }

    pub fn list<I, C>(mut self, kind: ListKind, items: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Inline>,
    {
        self.nodes.push(Node::List {
            kind,
            items: items
                .into_iter()
                .map(|item| item.into().into_nodes())
                .collect(),
        });
        self
    }

    /// Image on its own line, wrapped in a paragraph so it sits in the block flow.
    pub fn image<S: Into<String>, A: Into<String>>(self, src: S, alt: A) -> Self {
        self.paragraph(Inline::new().image(src, alt))
    }

    /// Image with a caption paragraph under it.
    pub fn captioned_image<S: Into<String>, A: Into<String>, I: Into<Inline>>(
        self,
        src: S,
        alt: A,
        caption: I,
    ) -> Self {
        self.image(src, alt)
            .paragraph(Inline::new().italic(caption))
    }

    pub fn video(mut self, video: Video) -> Self {
        self.nodes.push(Node::Video(video));
        self
    }

    /// Video with a caption paragraph under it.
    pub fn captioned_video<I: Into<Inline>>(self, video: Video, caption: I) -> Self {
        self.video(video).paragraph(Inline::new().italic(caption))
    }

    pub fn rule(mut self) -> Self {
        self.nodes.push(Node::Rule);
        self
    }

    pub fn raw<S: Into<String>>(mut self, raw: S) -> Self {
        self.nodes.push(Node::Raw(raw.into()));
        self
    }

    /// Push a pre-built node. The seam a higher-level builder uses for anything not covered above.
    pub fn node(mut self, node: Node) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn nodes<I: IntoIterator<Item = Node>>(mut self, nodes: I) -> Self {
        self.nodes.extend(nodes);
        self
    }

    /// Fold another builder's contents in, for composing sub-documents.
    pub fn extend(mut self, other: DocumentBuilder) -> Self {
        self.nodes.extend(other.nodes);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn build(self) -> Document {
        Document { nodes: self.nodes }
    }
}
