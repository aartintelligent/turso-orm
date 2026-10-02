---
name: docs-site
description: How the documentation site (Zensical with the Material theme, docs/ and mkdocs.yml, published to GitHub Pages) is built, how pages include code from the examples through snippet markers, and how to add or move a page. Load before editing anything under docs/, mkdocs.yml, or a function that a page includes.
---
# Documentation site

The site is the guide; rustdoc is the API reference. Do not duplicate rustdoc
content in a page and do not paste code: include it.

## Commands

```bash
just docs-serve      # live preview, http://127.0.0.1:8000
just docs-build      # strict build: broken link or missing snippet = failure
```

Both go through `uv run --group docs zensical`, so nothing is installed
globally. Zensical reads `mkdocs.yml` in MkDocs format; keep configuration
examples in that format. `theme.variant` is `modern`; `classic` restores the
Material for MkDocs look if a style regression ever needs it. Template
overrides are MiniJinja: filters and tests only, no Python function calls,
and the `pymdownx.emoji` extension must point at `zensical.extensions.emoji`.

## Layout

```
mkdocs.yml                     Zensical configuration in MkDocs format: theme, extensions, nav (Home, Overview, Documentation, Project)
overrides/home.html            the whole landing page: a full-viewport hero on assets/turso/hero.png, text top right, code window bottom right; content and footer blocks are empty
docs/assets/stylesheets/brand.css  the turso.tech palette (Aqua #4FF8D2, Dark Teal #183134, Bluewood #293945, Mirage #162129, Bunker #0D1318) mapped onto Material's variables; `primary`/`accent: custom` in mkdocs.yml hand the colours to it
docs/assets/stylesheets/home.css  styles scoped to the hero (`.tx-hero*`); the `.md-main` of the home page is hidden
docs/assets/turso/             the illustrated Turso logomark from https://turso.tech/brand (site logo and favicon) and the hero illustration `hero.png`, the site name is "Turso ORM"; keep the independence note next to them
docs/index.md                  front matter only (template: home.html); the landing page has no Markdown body
docs/overview/                 presentation pages, no code: why, features, use-cases, design, compatibility, roadmap
docs/guide/getting-started.md  getting started; the only place a code snippet appears outside the technical pages
docs/guide/*.md                entities, queries (with error handling), relations, transactions, migrations
docs/project/                  about, contributing, community, license
```

Every page in `nav` must exist and every page must be in `nav`; `strict`
reports both. The home page is the hero alone, like zensical.org: no
sections below it. The Overview and Project tabs are prose: they explain
and link, they do not show code. Code belongs under Documentation. The
examples are not documented on the site: pages link to the `examples/`
directory on GitHub and include code from it, nothing more. The site is deliberately small: fold a new topic into an
existing page before adding one, and never add a page that only links
elsewhere.

## Including code

In the Rust source, wrap the function (doc comment included) with markers:

```rust
// --8<-- [start:relations]
/// Walks relations in both directions.
async fn relations(db: &Database) -> Result<(), DbErr> { /* ... */ }
// --8<-- [end:relations]
```

In the page:

````markdown
```rust
--8<-- "examples/basic/src/query.rs:relations"
```
````

Rules:

- Markers wrap whole items so that the block compiles on its own in the
  reader's head. Indent the markers like the item (inside an `impl`, four
  spaces); rustfmt keeps them in place.
- A whole file is included without a marker: `--8<-- "path/to/file.rs"`.
- Line ranges (`file.rs:14:66`) are acceptable only for a file that is
  entirely an example, such as `crates/turso-orm/examples/quickstart.rs`.
- Markers are the single accepted exception to the "inline comments explain
  why" rule; do not add other tooling comments.
- When moving or renaming an included function, move its markers and run
  `just docs-build`.

## Turso branding

turso-orm is independent of Turso. Every place that shows the Turso logo or
name prominently (hero, the Turso card on the home page, README, footer
copyright) carries the sentence "not affiliated with or endorsed by Turso".
Keep it when editing those places. Logo files come from the brand kit and
are not modified; the palette in `brand.css` mirrors the brand page. The
site is dark only (a single `slate` palette, no toggle), like turso.tech;
do not reintroduce a light scheme without the maintainer's say.

## Writing style

Same as the rest of the repository: English, full sentences, code in
backticks. Use `!!! note` / `!!! warning` admonitions for Turso-specific
behaviour, `=== "axum"` / `=== "actix-web"` tabs when both frameworks are
shown, and a table for option lists. Mermaid `erDiagram` blocks render
without a plugin.
