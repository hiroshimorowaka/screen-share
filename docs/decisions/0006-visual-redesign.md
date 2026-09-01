# ADR-0006: Visual redesign — "Patchbay" identity, dark throughout

Date: 2026-08-31
Status: accepted — P1–P3 implemented on `feat/visual-redesign-patchbay`;
P4 deferred. The light lobby from decision 1 shipped in P1 and was then
reverted at the maintainer's request — everything is on the dark ground.

## Context

The UI works but has no identity of its own:

- The palette is a self-described Discord clone (`base.css`: *"the
  characteristic blurple"*, `--accent #5865f2`, `--bg #313338`).
- Typography is 100% the system stack with one size axis and no display
  face; `--font-mono` is declared but barely used.
- The home page is two stacked form panels plus a numbered 3-step list —
  no hero, no thesis.
- The room is functionally solid (adaptive grid, auto-hiding control bar,
  focus + filmstrip) but visually flat: `#000` grid, ~5 different
  ad-hoc `rgba(0,0,0,.55)` pill/badge treatments, 0.7–0.8rem text
  everywhere, and **three hover-reveal menus** that can open at once on a
  small tile (`menu-select` ×2 in the bar, `quality-menu` +
  `volume-control` per card). The maintainer has already called that
  stack "feio".
- The one thing that makes this app not Zoom/Meet — sharing and watching
  are independent, one P2P connection per (sharer, viewer) pair — is
  invisible in the UI.

A sibling project (`easyscreenshare`) already occupies the obvious
"AI-default" dark look: near-black `#0b0e14` + one hot accent
(`#ff5c38` signal orange) + Bricolage Grotesque. The redesign must not
land there or imitate it.

Research (Daily.co video-call UI teardown, Chrome screen-sharing UX docs,
2026 "AI slop" design write-ups) points the same way: in-call controls
stay accessible and labelled; only one contextual surface open at a time;
distinctiveness comes from a characterful display face, a small colour
system with its own rule repeated consistently, and structure that
exposes the data rather than decorating it.

## Decision

Adopt a **"Patchbay" / control-room** identity: the room is a wall of
monitors, and each "watch" is the viewer patching a cable to one screen.
Sharing and watching are different facts, so they get **two different
colours**, not one brand accent.

Maintainer choices (all five confirmed):

1. ~~**Light lobby, dark room.**~~ Tried in P1 (a route-scoped light
   `.lobby` / 404, dark room) and **reverted** — the maintainer didn't
   want the split. One dark ground for every route; the lobby and 404
   just add the wordmark + hero on top of it.
2. **External assets allowed.** The old "no external fonts/assets" rule
   is dropped (`CLAUDE.md` updated). Self-host web fonts; prefer proven
   libraries over reinventing them.
3. **Two signal colours; drop Discord blue entirely.**
   `--sig-live` (green) = *this person is sharing*; `--sig-patch`
   (violet) = *you are watching this / your connection*.
4. **Room rework is structural, not just paint.** Consolidate the
   hover menus and change the card metaphor — the "feio" complaint was
   about structure.
5. **"How it works" folds into the hero thesis** — no separate numbered
   step list as a visual centrepiece.

### Token system

#### Colour — one dark ground

| token | value | role |
|---|---|---|
| `--ink-0` | `#0d0f12` | grid void |
| `--ink-1` | `#16191e` | surface: header, card backing, control pills |
| `--ink-2` | `#20242b` | raised: popovers, pressed pill |
| `--border` | `rgba(255,255,255,.08)` | hairlines |
| `--text` / `--text-dim` | `#eceef1` / `#8a929d` | |
| `--sig-live` | `#46d17f` | someone is sharing (hot wire) |
| `--sig-patch` | `#8b7ff2` | you are watching this (your patch); also `--accent` |
| `--pill-bg` / `--pill-bg-strong` | `rgba(9,11,14,.62 / .82)` | badges/pills over video |
| `--warning` / `--error` | `#f0b232` / `#e0484d` | kept |

The generic names (`--surface`, `--accent`, …) that base.css is written
against resolve to these — see `apps/web/public/styles/tokens.css`.

#### Type

| role | face | delivery |
|---|---|---|
| display, wordmark, headings | **Space Grotesk** (500, 700), `-0.02em` | Google Fonts `<link>` in the document head, `display=swap` |
| data readouts — room code, `N/10`, ping, status labels, "AO VIVO" tag | **Space Mono** (400, 700), uppercase, `+0.04em` | same `<link>` |
| body, UI, form controls | system sans stack (current `--font-sans`) | — |

Space Grotesk is derived from Space Mono (same designer), so headline and
readout read as one family — the "instrument" tie. Both are OFL. Loaded
from Google Fonts (`fonts.googleapis.com` / `fonts.gstatic.com`) via a
`<link>` in `app.rs` `shell`, not vendored — the maintainer opted for the
CDN over checking woff2 into the repo. Type scale ratio 1.25:
`0.8 / 0.875 / 1.0 / 1.25 / 1.563 / 2.441rem`.

### Layout

**Lobby** — single column on the dark ground. Wordmark + a live
`N salas · M online` mono readout (real data from the recent-rooms
check). Hero is one plain declarative sentence — it names what the site
does for a first-time visitor ("Compartilhe sua tela com o grupo."), and
the lead line under it carries the distinctive part (any member
transmits, each viewer picks who to watch, browser-only, no host). Below:
`Criar` / `Entrar` cards, equal weight, side by side, stacking under
46rem. The 404 route reuses the same shell.

**Room** — grid / focus / filmstrip logic unchanged
(`recompute_adaptive_grid`, `.grid--focused`). Chrome recalibrated:

- thin header: lamp + room name + mono readouts + audio chip + invite;
  status sentence only when off-nominal (already so).
- **card = monitor**: the member colour becomes a 1px hairline border +
  a small name tab (an equipment label), replacing the 2px ring +
  `radial-gradient` glow — a calmer wall.
- one bottom-centre control cluster (auto-hide unchanged). The two
  `MenuSelect`s (video mode, audio quality) + the mute button collapse
  into **one "⚙ transmissão" popover**.
- per-card viewer controls: volume stays inline; quality moves to
  **click-open**, not hover — no more stacked hover menus on a tiny tile.
- a **"você"** marker on the viewer's own card (a host-less room needs
  self-orientation).
- one shared pill/badge token replaces the ~5 ad-hoc translucent-black
  treatments.

### Signature — "the patch"

Make the invisible P2P wire visible.

- **v1 (this redesign):** a card you're watching carries a `--sig-patch`
  tab/notch; a card that's sharing carries a `--sig-live` edge. Two
  colours, two facts.
- **v2 (later, optional):** an SVG connector line from your tile to the
  focused tile in `.grid--focused`.

### Rollout — phased, each phase independently reviewable

| Phase | Scope | Status |
|---|---|---|
| **P1 Foundation + Lobby** | `tokens.css`, Google Fonts `<link>`, rebuilt `base.css` + `home.css`, 3 steps folded into the hero, live `N salas · M online` readout wired from the existing recent-rooms data. (A light `.lobby` scope shipped here and was later reverted.) No room changes. | done |
| **P2 Room chrome** | `room.css` / `card.css` / `dev_preview.css` repainted on `--ink-*` / `--sig-*`; `#000`/`rgba(0,0,0,…)` one-offs replaced by `--pill-bg(-strong)`; card = flat monitor (hairline bezel, colour in the name-tab dot + avatar, not the ring); `--sig-live` top edge on a sharing card, `--sig-patch` bezel on one you're watching; `.card--self` "você" tab; header name/count in Space Mono uppercase; lamp `live` → `--sig-live`. | done |
| **P3 Consolidation** | New `components/transmission_menu.rs` — video mode + audio quality + mute folded into one hover/focus popover behind a sliders icon; the two `MenuSelect`s removed and `components/menu_select.rs` deleted (superseded). Per-card quality menu: `:hover` open dropped, `:focus-within` (click) kept. | done |
| **P4 (optional)** | SVG connector line from your tile to the focused tile in `.grid--focused`. | deferred |

Each phase verified: `cargo fmt --check`, `cargo clippy` (ssr workspace +
hydrate, `-D warnings`), `cargo test --workspace --features ssr`, the
WASM suite, `cargo leptos build`, Playwright (9/9), and a hand-check in
the browser. The redesign touched no e2e selectors — headings stayed
headings, `aria-label`s and panel text unchanged.

### Not carried out from the plan above

- The per-card **patch tab** landed as a bezel/edge treatment
  (`.card--patched` / `.card--live`), not a separate text tab — the
  coloured frame reads at tile size where a tab would not.
- `dev_preview.rs` keeps its own trimmed control bar (hide-idle,
  hide-preview, leave); it does not render the share button or the
  transmission menu, so those are hand-verified on the real room only.

## Consequences

- `CLAUDE.md` "Tech stack" styling line already relaxed. This ADR is the
  reference it points to.
- Fonts load from Google's CDN at runtime: two extra `preconnect`s and
  one render-path stylesheet request to `fonts.googleapis.com` /
  `fonts.gstatic.com`, and a third-party dependency on Google for every
  page load (a known privacy/GDPR consideration — accepted here over
  vendoring). If a Content-Security-Policy is ever added, it needs
  `style-src`/`font-src` entries for those hosts. Nothing is checked into
  the repo.
- One dark ground for every route: the live room grid is flat `--ink-0`;
  every other surface (lobby, 404, the pre-auth nick / not-found panels)
  sits on base.css's default body background (a faint `--sig-patch` glow
  over `--ink-0`). `body:has(.room-page:not(.hidden))` is what flips to
  the flat grid ground and zeroes the centering padding.
- Accessibility: `--text` on `--ink-*` clears AA; visible focus rings and
  `prefers-reduced-motion` handling are preserved. No light mode.
- `docs/architecture/overview.md` gains a short note on the token system.
- Motion budget stays small: one lobby hero reveal, the existing lamp
  pulse, card fade-in. No scroll effects.
