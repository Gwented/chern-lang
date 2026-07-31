# website

The landing page (`src/landing.rs`) is a hub: one bullet per site section, no content of its own.
Add a section by adding it to `landing::sections`.

Error codes are one such section. `errors::index` builds `errors/index.html`, the listing of every
code; `./src/pages/` contains the static pages the error codes render.

Images and video live in `site/resources/` and ship as-is — the generator doesn't copy them.
`src/resources.rs` spells the hrefs per page depth. Pages insert media with
`ErrorDocBuilder::{image, captioned_image, video, clip}`.
