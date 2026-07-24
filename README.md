# rust-site-template

A small personal-site template: **Rust + Axum + Tera + SQLite**.

No JavaScript framework, no build step, no web fonts. One inline stylesheet, light and dark
themes, and a single ~10 MB binary that serves the whole site.

- Pages: home, about, work, blog (database-backed), a project page, contact form
- Light/dark automatically, following the visitor's system setting
- Contact form saves to SQLite, with optional email + captcha
- Real 404s, security headers, and WCAG AA colour contrast out of the box

## Run it

```bash
cargo run
# open http://localhost:3000
```

That is the whole setup — SQLite creates itself on first run. To get some blog posts:

```bash
./seed.sh          # needs the sqlite3 CLI
```

## Make it yours

Almost everything you need to change is in `templates/`, and marked with `EDIT:` comments.

| What | Where |
|---|---|
| Site name, nav links, footer | `templates/base.html` |
| Colours and all styling | the `:root` block in `templates/base.html` |
| Intro and featured list | `templates/home.html` |
| Your bio | `templates/about.html` |
| Project list | `templates/work.html` |
| A project page | `templates/projects/example.html` |

### Adding a project page

1. Copy `templates/projects/example.html` to `templates/projects/yourthing.html`
2. In `src/main.rs`, find the `PROJECT PAGES` section and add:

```rust
.route("/projects/yourthing", get(project_yourthing))
```

3. Add the handler next to the others:

```rust
async fn project_yourthing(State((_, tera, _, _)): State<AppState>) -> (StatusCode, Html<String>) {
    simple_page(&tera, "projects/yourthing.html", "Your Thing")
}
```

If you forget step 2, the page returns a 404 — the template alone is not enough.

### Colours

The palette is CSS custom properties in `templates/base.html`, defined twice: once for dark
mode and once inside the `prefers-color-scheme:light` block. Change `--accent` for the quickest
restyle.

One caution: `--faint` is the lightest text on the page (nav, timestamps, footer). The shipped
values pass WCAG AA (4.5:1) against the background in both modes. If you lighten them, check the
contrast — this is the single easiest way to fail an accessibility audit.

## Configuration

Everything is optional; copy `.env.example` to `.env`.

| Variable | Default | Purpose |
|---|---|---|
| `PORT` | `3000` | Port to listen on |
| `TEMPLATE_DIR` | `templates` | Where templates live |
| `STATIC_DIR` | `static` | Where static files live |
| `DATABASE_PATH` | `data.db` | SQLite file |
| `SITE_URL` | `https://example.com` | Used in `robots.txt` |
| `TURNSTILE_SITE_KEY` | — | Shows the captcha widget |
| `TURNSTILE_SECRET_KEY` | — | Verifies the captcha server-side |
| `SENDGRID_API_KEY` | — | Emails contact submissions |
| `CONTACT_TO_EMAIL` | — | Where contact mail goes |
| `CONTACT_FROM_EMAIL` | — | Verified sender address |

With no captcha keys the form still works, and with no SendGrid key messages are still saved to
the database. Set them when you want them, not before.

**Note on the captcha:** a site key alone does nothing. The widget is cosmetic until
`TURNSTILE_SECRET_KEY` is set, because that is what makes the server actually verify the token.
Set both or neither.

## Deploying

Build, then run the binary next to `templates/` and `static/`:

```bash
cargo build --release
./target/release/rust-site-template
```

A systemd unit is in `website.service.example`. Put a reverse proxy (nginx, Caddy) or a
Cloudflare Tunnel in front for HTTPS.

Two things worth knowing:

- **Run it from the project root**, or set `TEMPLATE_DIR` and `STATIC_DIR`. The app refuses to
  start if it finds no templates, rather than starting and serving an error on every page.
- **Replacing a running binary** fails with `Text file busy`. Stop the service, copy, then start.

## Reading the code

`src/main.rs` is the whole app, about 350 lines, in sections: config, middleware, the router,
then handlers grouped by area. `src/db.rs` creates the schema, `src/models.rs` has the two row
types.

## License

Copyright (c) 2026 BuiltNotTaught

This work is licensed under a
[Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International License (CC BY-NC-ND 4.0)](https://creativecommons.org/licenses/by-nc-nd/4.0/).

You are free to **share** (copy and redistribute) this material as-is, with attribution, provided that:

- **Attribution** — you give appropriate credit to BuiltNotTaught.
- **NonCommercial** — you may not use the material for commercial purposes.
- **NoDerivatives** — you may not remix, transform, or build upon the material and distribute the modified work.

All rights not expressly granted are reserved. This applies to the entire work, including source code.
