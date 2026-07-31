//! Site generator. Writes a stylesheet, a landing hub, the error-code index, and one page per
//! error code under a site root.
//!
//! `cargo run -p website -- [site_root]` — defaults to `site/`, relative to the invocation.
//!
//! Page content lives in `website::pages`, one module per code. This file only walks them and
//! writes files.

use std::io;
use std::path::{Path, PathBuf};

use website::errors::ErrorDoc;
use website::renderers::html_renderer::HtmlRenderer;
use website::style;

fn main() -> io::Result<()> {
    let root: PathBuf = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("site"), PathBuf::from);

    println!("Generating pages:");
    let style_path = root.join(style::FILE_NAME);
    write(&style_path, style::STYLESHEET)?;
    dbg!(style::STYLESHEET);
    println!("{}", style_path.display());

    let landing_renderer = HtmlRenderer::page("chrn").with_stylesheet(style::LANDING_HREF);
    let landing_path = website::landing::output_path(&root, &landing_renderer);
    write(
        &landing_path,
        &website::landing::landing().render(&landing_renderer),
    )?;
    println!("{}", landing_path.display());

    let errors_renderer =
        HtmlRenderer::page("chrn error codes").with_stylesheet(style::ERRORS_INDEX_HREF);
    let errors_index_path = website::errors::index_output_path(&root, &errors_renderer);
    write(
        &errors_index_path,
        &website::errors::index().render(&errors_renderer),
    )?;
    println!("{}", errors_index_path.display());

    for doc in website::pages::all_pages() {
        let renderer = renderer(&doc);
        let path = doc.output_path(&root, &renderer);

        write(&path, &doc.render(&renderer))?;
        println!("{}", path.display());
    }

    Ok(())
}

/// Shell every error page shares. Depth-sensitive: pages sit at `errors/<label>/`.
fn renderer(doc: &ErrorDoc) -> HtmlRenderer {
    HtmlRenderer::page(format!("{} chrn", doc.label())).with_stylesheet(style::ERROR_PAGE_HREF)
}

fn write(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}
