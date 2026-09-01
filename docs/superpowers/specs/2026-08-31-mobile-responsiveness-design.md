# Mobile responsiveness — design

Date: 2026-08-31
Status: accepted, implementing on `feat/mobile-responsiveness`
Related: `docs/decisions/0006-visual-redesign.md` (the token system this
builds on), `docs/decisions/0007-mobile-responsiveness.md` (the ADR this
spec produces).

## Problem

The app has zero responsive breakpoints. On a phone:

- `.room-page` uses `height: 100vh`, which counts the viewport *without*
  the browser chrome, so the fixed bottom control bar sits behind the
  address bar / gesture area.
- The entire interaction model is hover / `mousemove`: `.card__actions`
  are `opacity: 0` until `.card:hover`; the control bar auto-hides on a
  `mousemove` window listener and never re-appears without one; the
  volume, quality and transmission menus open on `:hover` / `:focus-within`
  popovers anchored to a tiny tile corner; several tooltips are
  hover-only.
- No `env(safe-area-inset-*)` anywhere — the fixed bar collides with the
  iOS home indicator, the header with a landscape notch.
- The adaptive grid (`grid.rs`) picks column count from the container's
  aspect ratio; a tall portrait phone gets 1 column, so a room of 6–10
  members is a long vertical scroll of tiny tiles.
- `.field__input` is `font-size: 0.95rem` (~15 px), under the 16 px iOS
  Safari zooms-on-focus threshold.

**Constraint that shapes the whole design:** `getDisplayMedia` is
unavailable in every mobile browser (Chrome Android, Safari iOS). On a
phone a member can only *watch*, never share. Every sharer control is
dead weight there.

## Approach

**CSS-first, no forked components.** Same DOM, same Leptos components. The
behaviour adapts through:

- `@media` width queries for layout (lobby stacking, header compaction,
  grid gaps).
- `@media (hover: none) and (pointer: coarse)` for the touch variant
  (bottom sheets, target sizes, filmstrip direction, always-on card
  actions gated by a chrome-visibility class).
- One small `hydrate`-only JS seam: a `matchMedia("(hover: none) and
  (pointer: coarse)")` helper that drives an `is_touch: RwSignal<bool>`.
  Only two Leptos handlers read it (`card_click`, the auto-hide setup).

Rejected: pure CSS with no signal — the "tap the video toggles the
chrome" behaviour needs JS regardless, so `matchMedia` is the minimal
seam. Rejected: a dedicated mobile layout path / conditional markup —
more surface area than the problem needs.

## The mobile room model

On touch (`is_touch`):

- **Patch → focus.** Watching a sharer also sets `expanded = Some(peer_id)`
  — one video at a time. The roster grid stays as the "who's here /
  who's sharing" view you arrive at and tap from.
- **Filmstrip is a compact tray.** `.grid--focused` under the touch media
  query: smaller tiles (`minmax(4.5rem, 6rem)` columns, `4.25rem` rows),
  and the grid's existing `overflow-y` scrolls the overflow. Tapping a
  tray tile switches focus (already `card_click`'s behaviour for a video
  tile). A true sideways tray needs a wrapper around the non-focused
  cards and is out of scope for this CSS-only pass.
- **Tap the video toggles the whole chrome.** `controls_visible` is the
  single "chrome shown" flag: the bottom bar, the focused card's
  `.card__actions`, and its badges fade together. In focus mode a tap on
  the focused video (`card_click`) flips `controls_visible`; it also
  auto-hides ~3 s after it was last shown. Leaving focus is a back button
  in the bar, not a tap.
- **Roster chrome stays up.** Outside focus mode the bar is always visible
  on touch (it is two buttons); the auto-hide timer only runs in focus
  mode.

## Component / file changes

### Foundation (no behaviour change)

| File | Change |
|---|---|
| `apps/web/src/app.rs` | viewport meta gains `viewport-fit=cover` |
| `room.css` `.room-page` | `height: 100svh; height: 100dvh;` |
| `room.css` `.room-controls` | `bottom: calc(1.5rem + env(safe-area-inset-bottom))` |
| `room.css` `.stage-header` | `padding-inline: max(1.1rem, env(safe-area-inset-left)) max(1.1rem, env(safe-area-inset-right))` |
| `base.css` `.field__input` | `font-size: max(16px, 0.95rem)` |

### Touch detection

New `apps/web/src/features/room/touch.rs`:

- `#[cfg(feature = "hydrate")] setup_touch_signal(set_is_touch: WriteSignal<bool>)`
  — reads `matchMedia("(hover: none) and (pointer: coarse)")`, sets the
  signal now and on every `change` event (kept-alive listener via
  `Closure::forget`).
- `#[cfg(not(feature = "hydrate"))]` stub: no-op.

`RoomPage`: `let (is_touch, set_is_touch) = signal(false); setup_touch_signal(set_is_touch);`

### Room behaviour

- `MemberCardSignals` gains `is_touch: ReadSignal<bool>` and
  `controls_visible: RwSignal<bool>`.
- `card_click` (`member_card.rs`):
  - watch branch: after `watch(ev)`, if `is_touch`, `expanded.set(Some(peer_id))`.
  - focused-tile branch on touch: instead of collapsing, flip
    `controls_visible`; do not clear `expanded`.
- `setup_auto_hide_controls(controls_visible, is_touch, expanded)` (`grid.rs`):
  - keep the desktop `mousemove` → show + `schedule_hide` path.
  - Effect A (tracks `is_touch`, `expanded`): on touch, when not focused,
    `cancel_pending()` and force `controls_visible` true; when focused, do
    nothing (Effect B arms the hide).
  - Effect B (tracks `is_touch`, `expanded`, `controls_visible`): on
    touch, focused, and visible → `schedule_hide()`.
  - `dev_preview.rs` call site updated (pass a constant `false` signal and
    its own `expanded`).

### Grid in portrait

`best_column_count` (`grid.rs`): after picking the aspect-optimal count,
`if width < 560.0 && visible >= 3 { best.max(2) }` — a narrow screen with
3+ members never collapses to a single column. Unit test in
`grid_tests.rs` updated.

### Controls & menus on touch (`@media (hover: none) and (pointer: coarse)`)

- `.card--focus .card__actions`: `opacity: 0; pointer-events: none` by
  default; `.room-page:not(.chrome-hidden) .card--focus .card__actions`
  restores them. `.room-page` gets `class:chrome-hidden=move || !controls_visible.get()`.
- Badges (`.watcher-badge`, `.card__corner-start`, `.card__nick`) on a
  focused card fade with `.chrome-hidden`, mirroring the existing
  fullscreen `card--controls-idle` rule.
- Touch target sizes: `.icon-btn` → min 44 px; `.card__actions .icon-btn`
  → 44 px; `.color-swatch` → 2.75 rem; action `gap` bumped.
- Bottom bar: a back button (`×` / chevron) shown only when
  `expanded.is_some() && is_touch`, clears `expanded`. `bottom` already
  carries the safe-area inset from the foundation change.
- Header: under `max-width: 40rem`, tighter padding and `.invite-btn span
  { display: none }` (icon only). `share-chip` / `audio-chip` are already
  `is_sharing`-gated and never show on mobile.
- Quality picker → bottom sheet: `.quality-menu__popup` becomes
  `position: fixed; inset: auto 0 0 0; width: 100%; max-width: none;
  border-radius: 1rem 1rem 0 0;
  padding-bottom: calc(0.75rem + env(safe-area-inset-bottom));
  transform: translateY(100%)`, sliding to `translateY(0)` on the existing
  `:focus-within`. Rows enlarged (`padding: 0.9rem 1rem; font-size: 1rem`).
  No dim backdrop in v1.
- Volume on touch: the fader popover needs a sustained hover to reach, so
  `.volume-control__popup` is `display: none` and only the mute toggle
  stays. Volume *level* remains a desktop affordance.
- The transmission menu is sharer-only and can't render on a phone — no
  bottom-sheet treatment.
- Tooltips: `.watcher-badge` gets `tabindex="0"` and opens its tooltip on
  `:focus-within` under the touch query.

### Lobby / pre-auth

- `.field__input` 16 px (foundation) covers the nick gate and the
  create/join forms.
- `home.css` `.lobby__bar { flex-wrap: wrap }`.
- `base.css` `.panel { padding: clamp(1.25rem, 4vw, 1.75rem) }`.
- "Seu navegador não suporta compartilhar tela" (`mod.rs`): under touch,
  reword to a neutral "Compartilhar tela não é possível neste aparelho —
  você pode assistir normalmente" and drop the `status-text--error` red.

## Testing

- `playwright.config.ts`: new `projects` entry `mobile-web` — `viewport
  390×844`, `hasTouch: true`, `isMobile: true`, a mobile `userAgent`; keeps
  the fake-media Chrome args.
- New `apps/web/end2end/tests/room-mobile.spec.ts`, focused subset:
  - a tap on the focused video toggles `.room-controls--hidden` /
    `.chrome-hidden`.
  - watching a sharer lands directly in `.grid--focused` (focus mode).
  - the filmstrip container is horizontally scrollable
    (`scrollWidth > clientWidth` with 3+ members).
  - tapping the quality trigger opens the bottom-sheet popup; a tap
    outside closes it.
  - `.card__actions .icon-btn` bounding box ≥ 44×44 under the mobile
    project.
- Manual, recorded in the ADR "browser layer" section: iOS Safari on a
  real device (safe-area, the 16 px zoom, `dvh` behaviour on address-bar
  scroll), Android Chrome, widths 320 / 360 / 390 / 430, portrait and
  landscape.

## Rollout

Phased, each independently reviewable (same convention as ADR-0006):

| Phase | Scope |
|---|---|
| 1 | Foundation: viewport meta, `dvh`/`svh`, safe-area, input font size, the `is_touch` seam |
| 2 | Touch controls: `pointer: coarse` gating, always/tap-visible card actions, target sizes, `mousemove` → the touch auto-hide effects |
| 3 | Mobile room: patch → focus, horizontal filmstrip, chrome-on-tap, bar back button, `best_column_count` clamp |
| 4 | Bottom sheets + lobby / header polish + the reworded unsupported-share copy |
| 5 | `mobile-web` Playwright project + `room-mobile.spec.ts` |

Produces `docs/decisions/0007-mobile-responsiveness.md`.

## Non-goals

- A dedicated mobile layout / conditional markup path.
- "One video at a time" as a distinct navigation flow beyond patch → focus.
- Screen sharing from a phone (not possible in a mobile browser).
- A dim backdrop behind the bottom sheets (v2 if it reads as needed).
- Landscape-specific behaviour beyond focus mode already filling the
  container and the safe-area inset on the header.
