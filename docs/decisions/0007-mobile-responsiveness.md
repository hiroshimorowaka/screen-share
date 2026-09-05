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
  touch. *(Revised 2026-09-04 — see the follow-up below; touch now gets a
  bottom-sheet fader.)*
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

## Follow-up (2026-09-04): touch fullscreen fixed; PiP dropped on touch

Round 2 of the mobile fixes
(`docs/superpowers/plans/2026-09-02-mobile-ui-fixes.md`) found the card's
"Tela cheia" button was a trap on touch: after entering fullscreen, the
idle-hidden controls could only be brought back by a tap, and that tap
(`card_click`) *exited* fullscreen instead — the `mousemove` reveal in
`setup_fullscreen_autohide_controls` never fires without a pointer.

Fixed by keeping fullscreen available on touch and changing the tap
semantics there:

- `card_click` on touch calls `reveal_fullscreen_controls_if_active()`
  instead of `exit_fullscreen_if_active()` — it dispatches a synthetic
  `mousemove` so the existing autohide wiring reveals the controls and
  re-arms its timer, and never leaves fullscreen. Entering / leaving
  fullscreen on touch is the "Tela cheia" button alone.
- `.card:fullscreen .card__actions` is shown on touch independent of the
  `chrome-hidden` focus-mode flag, gated only by `card--controls-idle`.
- **Picture-in-picture is hidden on touch** (`is_touch` gate in
  `participant/action_bar.rs`) — a phone browser has no PiP window, so the
  button was a silent no-op.

Desktop fullscreen behaviour (a click backs out of fullscreen) is
unchanged.

## Follow-up (2026-09-04): per-stream volume reachable on touch

The original decision left touch with mute/unmute only. That is too blunt
once a viewer watches several sharers that each carry their own audio
(ADR-0009): the phone's own volume rocker is a single global control and
can't balance one loud stream against a quiet one.

The fader is the *exact same* component and CSS on touch as on desktop —
`.volume-control` / `.volume-control__popup` / `.volume-control__slider`
carry no touch-specific rule at all, no bottom sheet, unlike the quality
picker. Only the trigger button's click semantics differ, in
`VolumeControl` (`participant/parts.rs`):

- **Desktop, unchanged.** Hover reveals the popup; clicking the button
  mutes/unmutes instantly, exactly as before.
- **Touch, no hover to reveal it.** The *first* tap on the button only
  opens the popup — the tap's own default focus is what CSS
  `:focus-within` (already unconditional on the popup) reacts to, so
  nothing has to happen in the click handler beyond letting it through.
  Distinguishing "this tap is opening it" from "it was already open" reads
  the trigger's `mousedown`, before the browser's own focus-on-mousedown
  applies (`event_target_already_focused`, the same trick `QualityMenu`
  uses for its own open/close).
- **Every tap after that, while still open, mutes/unmutes** — the same
  action clicking the button while hovering it performs on desktop — and
  leaves the popup open, so the next thing the viewer does is drag the
  slider. `apply_mute_toggle` (`watch_widgets.rs`) skips its blur on touch
  for exactly this: blurring would close the popup the same tap just used
  to mute.
- **Closing it is never a second tap on the trigger.** An outside tap
  blurs it (same as a mouse losing hover), and so does the surrounding
  chrome fading on its own after a few idle seconds
  (`setup_auto_hide_controls`, `participant_grid/mod.rs`) — `.volume-control`
  lives inside `.card__actions`, whose opacity/`pointer-events` already
  compound over anything open inside it, no extra wiring needed.
- The slider still writes through to `volume_by_peer` / the real `<video>`
  on `input`; on touch it does **not** blur on `change`, so fine-tuning
  the level doesn't dismiss the popup either.
