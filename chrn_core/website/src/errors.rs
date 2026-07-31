//! Error-code documentation. Sits on top of [`crate::doc_builder`] and speaks in `chrn` terms:
//! named sections, `chrn` code blocks, cross-links between error pages.
//!
//! Page authors call `.summary(..)`, `.see_also(..)` — never a heading level or a `Node`.

use std::path::{Path, PathBuf};

use chrn_utils::err_codes::{self, ErrorCode, MAX_ERR_CODE_WIDTH};

use crate::doc_builder::{Document, DocumentBuilder, Inline, Video};
use crate::renderers::Renderer;
use crate::resources;

/// Directory the generated error pages live under, relative to the site root.
pub const ERRORS_DIR: &str = "errors";

/// Heading level for the page title.
const TITLE_LEVEL: u8 = 1;
/// Heading level for preset sections.
const SECTION_LEVEL: u8 = 2;
/// Heading level for a subsection inside a preset section.
const SUBSECTION_LEVEL: u8 = 3;

// Maybe put this in err_code
/// Title for error codes
pub fn error_title(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::ConfigLoadErr => "Config load error",
        ErrorCode::CompilerSafetyLimits => "Compiler safety limit",
        ErrorCode::SchemaOptionErr => "Schema option error",
        ErrorCode::ScopeErr => "Scope error",
        ErrorCode::DirectiveErr => "Directive error",
        ErrorCode::PrivacyErr => "Privacy error",
        ErrorCode::GenericsErr => "Generics error",
        ErrorCode::ConfigDeclErr => "Config declaration error",
        ErrorCode::ImportErr => "Import error",
    }
}

/// Href of the error index from the site root.
pub fn index_root_href() -> String {
    format!("{ERRORS_DIR}/")
}

/// Href of an error page from the site root.
pub fn root_href(code: ErrorCode) -> String {
    format!("{ERRORS_DIR}/{}/", err_codes::fmt_err_code(code))
}

/// Href of an error page from the error index.
pub fn index_href(code: ErrorCode) -> String {
    format!("{}/", err_codes::fmt_err_code(code))
}

/// Path of the error index under a site root, e.g. `site/errors/index.html`.
pub fn index_output_path<R: Renderer>(root: &Path, renderer: &R) -> PathBuf {
    root.join(ERRORS_DIR)
        .join(format!("index.{}", renderer.extension()))
}

/// The error section index: one link per code in [`ALL_ERROR_CODES`] order. Titles come from
/// [`error_title`], so a new code appears here for free.
pub fn index() -> Document {
    let err_code_bullets = err_codes::ALL_ERROR_CODES.into_iter().map(|code| {
        Inline::new().link(
            index_href(code),
            Inline::new()
                .code(err_codes::fmt_err_code(code))
                .text(format!(" {}", error_title(code))),
        )
    });

    Document::builder()
        .heading(TITLE_LEVEL, "error codes")
        .paragraph(
            Inline::new()
                .text("Every code the compiler emits. Codes are category-level, so one page covers a range of diagnostics."),
        )
        .video(Video::new(format!("../{}/email_gmail.mp4", resources::RESOURCES_DIR), ""))
        .bullets(err_code_bullets)
        .build()
}

/// Language tag attached to a code block. Keeps `"chrn"` from being retyped on every page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLang {
    /// A `.chrn` script.
    Chrn,
    /// Serialized data the script describes.
    Serial,
    /// Compiler output, shell transcripts, anything unhighlighted.
    Plain,
}

impl CodeLang {
    fn tag(self) -> Option<String> {
        match self {
            CodeLang::Chrn => Some("chrn".into()),
            CodeLang::Serial => Some("serial".into()),
            CodeLang::Plain => None,
        }
    }
}

/// Finished document that was made with templated `ErrorDocBuilder` specific methods
#[derive(Debug, Clone)]
pub struct ErrorDoc {
    code: ErrorCode,
    document: Document,
}

impl ErrorDoc {
    /// Start a page. The title heading is emitted immediately, so section presets can assume it.
    pub fn builder(code: ErrorCode) -> ErrorDocBuilder {
        ErrorDocBuilder::new(code)
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// `E0004`
    pub fn label(&self) -> String {
        err_codes::fmt_err_code(self.code)
    }

    pub fn title(&self) -> &'static str {
        error_title(self.code)
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn render<R: Renderer>(&self, renderer: &R) -> String {
        renderer.render(&self.document)
    }

    /// Path of this page's file under a site root, e.g. `errors/E0004/index.html`.
    pub fn output_path<R: Renderer>(&self, root: &Path, renderer: &R) -> PathBuf {
        root.join(ERRORS_DIR)
            .join(self.label())
            .join(format!("index.{}", renderer.extension()))
    }

    /// Href another error page uses to link here, relative to a sibling error page.
    pub fn sibling_href(code: ErrorCode) -> String {
        format!("../{}/", err_codes::fmt_err_code(code))
    }
}

/// High level builder over `DocumentBuilder` to reduce unneeded boiler-plate when docs are built
/// from conventional pieces of general markdown concepts.
#[derive(Debug, Clone)]
pub struct ErrorDocBuilder {
    code: ErrorCode,
    body: DocumentBuilder,
}

impl ErrorDocBuilder {
    fn new(code: ErrorCode) -> Self {
        let title = format!("{}: {}", err_codes::fmt_err_code(code), error_title(code));
        Self {
            code,
            body: Document::builder().heading(TITLE_LEVEL, title),
        }
    }

    // -- Prose --

    /// One-paragraph statement of what the error means. Goes directly under the title.
    pub fn summary<I: Into<Inline>>(mut self, text: I) -> Self {
        self.body = self.body.paragraph(text);
        self
    }

    /// Paragraph inside whichever section is currently open.
    pub fn prose<I: Into<Inline>>(mut self, text: I) -> Self {
        self.body = self.body.paragraph(text);
        self
    }

    /// `## Explanation` plus its body.
    pub fn explanation<I: Into<Inline>>(self, text: I) -> Self {
        self.section("Explanation").prose(text)
    }

    /// `## Cause` what the compiler was doing when it emitted this.
    pub fn cause<I: Into<Inline>>(self, text: I) -> Self {
        self.section("Cause").prose(text)
    }

    /// `## Fix` prose form
    pub fn fix<I: Into<Inline>>(self, text: I) -> Self {
        self.section("Fix").prose(text)
    }

    /// Bulleted list
    pub fn bullets<I, C>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Inline>,
    {
        self.body = self.body.bullets(items);
        self
    }

    /// `**Note:** ..` paragraph.
    pub fn note<I: Into<Inline>>(mut self, text: I) -> Self {
        self.body = self
            .body
            .paragraph(Inline::new().bold("Note: ").join(text.into()));
        self
    }

    // -- Sections --

    /// `## <name>`. Escape hatch for a section with no preset.
    pub fn section<I: Into<Inline>>(mut self, name: I) -> Self {
        self.body = self.body.heading(SECTION_LEVEL, name);
        self
    }

    /// `### <name>`, for splitting a preset section.
    pub fn subsection<I: Into<Inline>>(mut self, name: I) -> Self {
        self.body = self.body.heading(SUBSECTION_LEVEL, name);
        self
    }

    /// Thematic break between sections.
    pub fn divider(mut self) -> Self {
        self.body = self.body.rule();
        self
    }

    // -- Code --

    /// `chrn` code block with no surrounding heading.
    pub fn chrn<S: Into<String>>(self, code: S) -> Self {
        self.code_block(CodeLang::Chrn, code)
    }

    /// `## Wrong example` plus a `chrn` block.
    pub fn wrong_example<S: Into<String>>(self, code: S) -> Self {
        self.section("Wrong").chrn(code)
    }

    /// `## Corrected example` plus a `chrn` block.
    pub fn correct_example<S: Into<String>>(self, code: S) -> Self {
        self.section("Correct").chrn(code)
    }

    /// `## Serialized data` plus the data a script describes.
    pub fn serialized<S: Into<String>>(self, data: S) -> Self {
        self.section("Serialized data")
            .code_block(CodeLang::Serial, data)
    }

    /// `## Diagnostic` plus compiler output, verbatim.
    pub fn diagnostic<S: Into<String>>(self, output: S) -> Self {
        self.section("Diagnostic")
            .code_block(CodeLang::Plain, output)
    }

    /// Code block in an explicit language.
    pub fn code_block<S: Into<String>>(mut self, lang: CodeLang, code: S) -> Self {
        self.body = self.body.code_block(lang.tag(), code);
        self
    }

    // -- Media --
    //
    // `name` is a file in `site/resources/`; the href is resolved for an error page's depth by
    // [`crate::resources::error_page_href`], so pages never spell `../../`.

    /// Image from `site/resources/`, on its own line.
    pub fn image<A: Into<String>>(mut self, name: &str, alt: A) -> Self {
        self.body = self.body.image(resources::error_page_href(name), alt);
        self
    }

    /// Image from `site/resources/` with a caption under it.
    pub fn captioned_image<A: Into<String>, I: Into<Inline>>(
        mut self,
        name: &str,
        alt: A,
        caption: I,
    ) -> Self {
        self.body = self
            .body
            .captioned_image(resources::error_page_href(name), alt, caption);
        self
    }

    /// Video from `site/resources/`, with controls. `label` is the fallback text.
    pub fn video<L: Into<String>>(self, name: &str, label: L) -> Self {
        self.video_with(Video::new(resources::error_page_href(name), label))
    }

    /// Silent looping clip from `site/resources/` — a demo, not something to sit through.
    pub fn clip<L: Into<String>>(self, name: &str, label: L) -> Self {
        self.video_with(Video::new(resources::error_page_href(name), label).looping_clip())
    }

    /// Pre-built [`Video`], for playback settings the presets don't cover. The caller owns the
    /// href.
    pub fn video_with(mut self, video: Video) -> Self {
        self.body = self.body.video(video);
        self
    }

    /// Video with a caption under it.
    pub fn captioned_video<I: Into<Inline>>(mut self, video: Video, caption: I) -> Self {
        self.body = self.body.captioned_video(video, caption);
        self
    }

    // -- Links --

    /// `## See also` plus a bullet list linking sibling error pages.
    pub fn see_also<I: IntoIterator<Item = ErrorCode>>(self, codes: I) -> Self {
        let items: Vec<Inline> = codes
            .into_iter()
            .map(|code| {
                Inline::new().link(
                    ErrorDoc::sibling_href(code),
                    Inline::new()
                        .code(err_codes::fmt_err_code(code))
                        .text(format!(": {}", error_title(code))),
                )
            })
            .collect();

        self.section("See also").bullets(items)
    }

    /// `## Reference` plus a link into the language spec.
    pub fn spec_reference<S: Into<String>, I: Into<Inline>>(mut self, href: S, label: I) -> Self {
        self.body = self
            .body
            .heading(SECTION_LEVEL, "Reference")
            .paragraph(Inline::new().link(href, label));
        self
    }

    /// Drop to the backend for anything the presets don't cover.
    pub fn with_document<F: FnOnce(DocumentBuilder) -> DocumentBuilder>(mut self, f: F) -> Self {
        self.body = f(self.body);
        self
    }

    pub fn build(self) -> ErrorDoc {
        ErrorDoc {
            code: self.code,
            document: self.body.build(),
        }
    }
}
