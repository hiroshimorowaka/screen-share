# ADR-0007: Mobile responsiveness — CSS-first, watch-only on touch

Date: 2026-08-31
Status: accepted — implemented on `feat/mobile-responsiveness`.
Design: `docs/superpowers/specs/2026-08-31-mobile-responsiveness-design.md`.
Builds on: ADR-0006 (the token system and the room chrome this adapts).

## Context

The app shipped with no responsive breakpoints. On a phone the room was
unusable in several concrete ways: `100vh` put the fixed control bar
behind the address bar; every control was hover- or `mousemove`-driven
(card actions, the volume / quality / transmission popovers, the
auto-hiding bar) and touch has neither; no `env(safe-area-inset-*)`, so
the bar and header collided with the notch / home indicator; the adaptive
grid collapsed to one column in portrait; and form inputs under 16px made
iOS Safari zoom on focus and never zoom back.

The fact that drives the shape of the solution: **`getDisplayMedia` is
unavailable in every mobile browser** (Chrome Android, Safari iOS). On a
phone a member can only watch, never share.

## Decision

**CSS-first, no forked components.** The same DOM and the same Leptos
components adapt through `@media` width queries, a
`@media (hover: none) and (pointer: coarse)` touch variant, and one small
JS seam — a `matchMedia` helper (`features/room/touch.rs`) driving an
`is_touch` signal that two handlers read.

### Foundation

- `viewport-fit=cover` on the viewport meta; `env(safe-area-inset-*)` on
  the fixed `.room-controls` (bottom) and `.stage-header` (inline).
- `.room-page` height: `100vh` → `100svh` → `100dvh` cascade.
- `.field__input` `font-size: max(16px, 0.95rem)`.

### The mobile room (touch only)

- **Patch → focus.** Watching a sharer also sets `expanded` — one video at
  a time. The roster grid stays as the "who's here / who's sharing" view.
- **Tap the video toggles the whole chrome.** `controls_visible` is the
  single "chrome shown" flag (`.room-page.chrome-hidden`); the bottom bar,
  the focused card's actions, and its badges fade together. `card_click`
  flips it in focus mode; it auto-hides ~3s after last shown. Leaving
  focus is a back button in the bar (`icon_minimize`), shown only on
  touch + in focus.
- **Roster chrome stays up.** The auto-hide timer only runs in focus mode
  on touch.
- The quality picker becomes a bottom sheet (`position: fixed`, slides up
  from the bottom edge, big rows), opened by the existing `:focus-within`.
- Touch targets: `.icon-btn` ≥ 44px, card-action buttons 2.75rem, bar
  buttons 3.25rem, swatches 2.75rem.
- `best_column_count` never collapses below 2 columns on a screen under
  560px with 3+ members.

### Narrow-screen layout (width query, not touch-specific)

- Header compacts under 40rem; the invite button goes icon-only.
- `.panel` padding `clamp(1.25rem, 4vw, 1.75rem)`; `.lobby__bar` wraps.
- The "can't screen-share" line reads as a neutral note (not red) on
  touch — it isn't an error there.

## Carried differently from the design

- **Filmstrip stays a compact vertical tray**, not a horizontal scroll
  strip. A sideways tray needs a wrapper element around the non-focused
  cards; this CSS-only pass doesn't add markup. The tray tiles shrink
  (`minmax(4.5rem, 6rem)` columns, `4.25rem` rows) and the grid's existing
  `overflow-y` scrolls the overflow.
- **Per-stream volume on touch is the mute toggle only** — the vertical
  fader popover needs a sustained hover to reach. Volume *level* stays a
  desktop affordance; the `.volume-control__popup` is `display: none` on
  touch.
- **The transmission menu gets no bottom-sheet treatment** — it is
  sharer-only and a phone can't share, so it never renders there.

## Testing

- New Playwright project `mobile-web` (`viewport 390×844`, `hasTouch`,
  `isMobile`, mobile UA) running only `tests/room-mobile.spec.ts`: patch
  → focus, tap toggles the chrome, the quality bottom sheet opens /
  closes, action buttons ≥ 44px. The historical `desktop` project runs
  everything else.

## Browser layer — still hand-verified

Not automatable here; check on a UI-touching mobile change:

- iOS Safari on a real device: safe-area insets clear the home indicator
  and a landscape notch; no zoom on input focus; `dvh` behaviour as the
  address bar scrolls.
- Android Chrome: the same, plus the gesture bar.
- Widths 320 / 360 / 390 / 430, portrait and landscape.
- The bottom sheet's slide-up and outside-tap close on a real touch
  screen (Playwright taps are synthetic).

## Consequences

- One new `hydrate`-only module (`features/room/touch.rs`) and one new
  web-sys feature (`MediaQueryList`).
- `MemberCardSignals` gains `is_touch` and `controls_visible`;
  `setup_auto_hide_controls` gains `is_touch` + `expanded`.
- `docs/architecture/overview.md` unchanged — no architectural shift, this
  is presentation-layer only.
- No light mode, no new motion beyond the sheet slide (respects
  `prefers-reduced-motion` via the existing global rule — the sheet uses
  `transform`, which that rule doesn't currently disable; acceptable, it's
  a short 0.15s move).
