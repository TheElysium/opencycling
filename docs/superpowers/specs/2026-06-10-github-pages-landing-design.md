# GitHub Pages Landing Page — Design

**Date:** 2026-06-10
**Status:** Approved design, ready for implementation plan
**Branch:** `github-page`

## Goal

A single-page marketing site for OpenCycling, published via GitHub Pages, that
presents the app honestly and matches the desktop app's visual identity. It is a
static page with no build step, decoupled from the Tauri/SvelteKit app build.

## Positioning

OpenCycling is presented as a **focused, no-frills ERG workout player** you own.

- Tagline angle: "Just your structured ERG sessions: no avatars, no worlds, no
  subscription. Does one thing, does it well."
- **Do not** describe it as an "alternative to Zwift / TrainerRoad". Those tools
  offer far more (virtual worlds, social, SIM). OpenCycling is a deliberately
  smaller, complementary tool — that framing is a feature, not a shortfall.
- Be honest about scope: **ERG mode only**, no SIM / free-ride / manual
  resistance.

## Visual identity (matches the app)

Reuse the app's exact design tokens (from `src/app.css`):

| Token | Value | Use |
|---|---|---|
| `--bg` | `#f8f9fa` | page background |
| `--surface` | `#ffffff` | cards |
| `--surface-dark` | `#1e293b` | workout-chart visual blocks |
| `--border` | `#e2e8f0` | card borders |
| `--text` | `#1a202c` | body text |
| `--muted` | `#718096` | secondary text |
| `--accent` | `#3b82f6` | brand wordmark, primary CTA |
| zones `--z1..z6` | `#94a3b8 #60a5fa #4ade80 #facc15 #fb923c #f87171` | zone bars / badges |

- Font: `system-ui, -apple-system, sans-serif`.
- Card radius 10px, button radius 7px, primary button = solid `--accent` white text.
- Light-only (`color-scheme: light`), no dark mode.
- Brand wordmark: `OPENCYCLING` in `--accent`, bold, slight letter-spacing,
  preceded by the 4-color logo block (green/blue/yellow/red) or `logo-source.svg`.

## Page structure (single scroll, top to bottom)

1. **Hero** *(locked in brainstorm)*
   - Headline: "Structured ERG workouts. Your trainer, your data."
   - Sub: connect a smart trainer over BLE, run Zwift `.zwo` workouts in ERG,
     review every session. No account, no subscription.
   - Pill: `100% OFFLINE · OPEN-SOURCE`.
   - CTAs: primary "Download for Windows" + secondary "View on GitHub".
   - Visual: a workout card rendered exactly like the app (dark `#1e293b` chart
     with zone bars + SWEET SPOT badge + `1h 5m · 57 TSS · 0.72 IF` line).
   - Top nav: Features · Compatibility · FAQ + GitHub star link.

2. **What it is** — one short paragraph, focused/no-frills positioning above,
   with an explicit "ERG only, no SIM/free-ride" honesty note.

3. **Features** — 3–4 cards with icons:
   - Automatic BLE scanning (separate trainer / HRM status).
   - Zwift `.zwo` workout support (Warmup, SteadyState, IntervalsT, Cooldown).
   - ERG control per block (per-second target power, keep-alive).
   - Session history & review.

4. **Screenshots** — 2–3 real app screenshots: Workouts grid, live Session,
   History. **Dependency:** no real PNGs exist in the repo yet (only a
   placeholder in `docs/screenshots/`). Must be captured before launch.

5. **Compatibility** — device table reused from README (Decathlon D500 trainer,
   Polar HRM, generic FTMS/HRS "should work, untested") + the Windows / name-prefix
   filtering note.

6. **Download / Get started** — prominent download button + a "build from source"
   link (`pnpm tauri build`) for non-Windows users.

7. **Footer** — license, GitHub link, short tagline.

## Hosting

- **Static `index.html` in `/docs`**, published via GitHub Pages
  (Settings → Pages → branch `main`, folder `/docs`).
- Live URL: `https://theelysium.github.io/opencycling/`.
- No build step; the page must be openable directly as a file.
- Assets (logo SVG, screenshots, any CSS/JS) live under `/docs` (e.g.
  `docs/assets/` and the existing `docs/screenshots/`).
- The page is fully decoupled from the Tauri/SvelteKit app build.

### Note on the existing `/docs` contents

`/docs` already holds specs, the PRD, and handoff notes. Publishing from `/docs`
makes those technically reachable, but only `index.html` + linked assets are
surfaced. No action required, but keep the landing assets in a clear subfolder.

## Download link

- The CTA links **directly to the latest release installer asset**.
- **Constraint:** Tauri's default Windows bundles embed the version in the
  filename (e.g. `opencycling_0.1.0_x64-setup.exe`,
  `opencycling_0.1.0_x64_en-US.msi`), so a generic
  `releases/latest/download/<name>` URL is **not** stable across versions.
- **Decision:** pin the link to the current asset name and update it on each
  release, with a code comment marking it as release-coupled. (Future option:
  configure a version-less asset name in the release workflow to make the link
  permanent — out of scope here.)
- **Dependency:** requires at least one published GitHub Release with a Windows
  installer. If none exists at build time, the link points to the Releases page
  as a fallback until the first release ships.

## Implementation constraints

- Single self-contained `index.html` (inline `<style>` is acceptable to keep it
  zero-build and portable; may split a `styles.css` alongside it).
- Responsive: readable on mobile (stacked) and desktop (two-column hero,
  multi-column feature/screenshot grids), mirroring the app's breakpoints where
  sensible.
- All page copy in **English** (project docs convention).
- **Source of truth for all factual copy = the code + `README.md`.** Do NOT use
  `docs/prd.md` — it is outdated. Verify feature claims, supported `.zwo` blocks,
  and device compatibility against the actual code and README before writing them.
- No analytics, no external trackers, no web fonts that phone home (use system
  fonts) — consistent with the app's privacy/offline ethos.

## Out of scope

- Dark mode.
- Multi-page site / blog / docs portal.
- A build pipeline or static-site generator.
- Automating the download link's version (noted as a future option only).

## Open dependencies (not blockers for the plan, needed before launch)

1. Real app screenshots (Workouts, Session, History) saved under
   `docs/screenshots/`.
2. At least one GitHub Release with a Windows installer for the direct download
   link (otherwise fall back to the Releases page).
