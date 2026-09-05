# Mobile UI fix plan — web app (round 2)

> **For agentic workers:** phased plan, execute in order, one branch + one
> PR per phase. Read `CLAUDE.md` and `RUST_GUIDELINES.md` in full first.
> Language policy applies (English in code/docs/CI, pt-BR only in
> conversation). Commits are maintainer-gated. Every change ships with a
> test at the right layer (here: mostly Playwright under the `mobile-web`
> project in `apps/web/end2end/`, plus native `grid` unit tests).

**Goal:** close the touch/narrow-viewport bugs left open after the first
mobile pass (ADR-0007). Desktop was audited in the same sweep and is
clean — no desktop changes are in scope. All findings below are on touch /
narrow viewports.

## Implementation status (2026-09-04, branch `fix/mobile-ui-round-2`)

Landed together (one branch, `scripts/test-all.sh --no-mutants` green):

- **A1** — quality bottom sheet gets `z-index: 50` above `.room-controls`,
  a `:has(.quality-menu:focus-within)` fade of the bar, and a dim
  `::after` scrim. A2 was already resolved by ADR-0007
  (`.volume-control__popup { display: none }` on touch).
- **A2 (extended)** — went further than "mute-only is fine": the volume
  fader is now reachable on touch as the *exact same* small floating popup
  desktop already has — no bottom sheet and no touch-only CSS at all (two
  earlier passes tried both and were reverted per review). Only the
  trigger button's click semantics change on touch: first tap opens the
  popup (via focus + the already-unconditional `:focus-within`), every tap
  after that mutes/unmutes without closing it, and only an outside tap or
  the chrome's own idle fade closes it. Rationale + mechanics in
  ADR-0007's 2026-09-04 follow-up. `.volume-control__popup` is no longer
  `display: none` on touch.
- **A3 (roster only)** — `--controls-reserve` custom property +
  `padding-bottom` on `.grid:not(.grid--focused)` on touch, so the last
  roster row clears the permanently-visible bar. Focus-mode filmstrip vs.
  bar left as-is (the bar auto-hides there).
- **A4/A5 → replaced by a smaller fix per the maintainer.** OS fullscreen
  stays available on touch; Picture-in-picture is hidden there (`is_touch`
  gate in `participant/action_bar.rs`). The trap is fixed by changing the
  tap semantics: `card_click` on touch calls
  `reveal_fullscreen_controls_if_active()` (dispatches a synthetic
  `mousemove` so `setup_fullscreen_autohide_controls` reveals the controls
  and re-arms its timer) instead of `exit_fullscreen_if_active()`. Leaving
  fullscreen on touch is the "Tela cheia" button alone. See ADR-0007
  follow-up. Desktop behaviour unchanged.
- **B2** — filmstrip tiles on touch: `.card__corner-start` hidden, avatar
  1.75rem, tighter nick pinned to the bottom.
- **C2** — `.card__watch-pill` wraps (no ellipsis) in the narrow roster
  grid.
- **C1** — verified already fixed (the centre-offset math + the ≥2-column
  floor cover it); a `mobile-web` case locks it in.

**Deferred:**

- **B1 (focused video does not fill the phone width).** The CSS-only
  approaches tried (`1fr` filmstrip tracks; an absolutely-positioned
  `.card--focus`) both broke focus-mode grid placement — a single
  filmstrip card rendered as a full-height centre strip over the focused
  video. Reverted. This needs the **wrapper-element** rewrite (§2.1
  "Proper alternative"): wrap the non-focused cards in a real
  `.filmstrip` element so the focused area and the tray are laid out
  independently. Schedule separately.

## Context this plan now sits on top of

Two things landed after this plan was first drafted; the plan has been
re-based onto them.

1. **ADR-0007 "Mobile responsiveness"** (`docs/decisions/0007-mobile-responsiveness.md`,
   accepted) shipped the first pass: `dvh` height cascade,
   `env(safe-area-inset-*)`, the `is_touch` `matchMedia` seam, patch →
   focus, tap-toggles-chrome via a `controls_visible` signal +
   `.room-page.chrome-hidden`, the quality bottom sheet, ≥44px targets, and
   `best_column_count` never collapsing below 2 columns under 560px with
   3+ members. The `mobile-web` Playwright project and
   `apps/web/end2end/tests/room-mobile.spec.ts` **already exist** — Phase
   tests below *extend* that file, they do not create it.

2. **The 8-phase structure refactor** (`f63cbcb`) moved every file this
   plan referenced. The room UI now lives under `apps/web/src/room/`, not
   `apps/web/src/features/room/`. Concretely:

   | This plan's old path | Current path |
   |----------------------|--------------|
   | `src/features/room/member_card.rs` | `src/room/components/participant/mod.rs` (card shell + `card_click`); the fullscreen / PiP buttons are in `src/room/components/participant/action_bar.rs`; the quality / volume widgets in `src/room/components/participant/parts.rs` + `watch_widgets.rs` |
   | `src/features/room/media_controls.rs` | `src/room/media_controls.rs` |
   | `src/features/room/grid.rs` | `src/room/components/participant_grid/mod.rs` (`setup_auto_hide_controls`, `setup_adaptive_grid`, `best_column_count`, `recompute_adaptive_grid`) |
   | `src/features/room/touch.rs` | `src/room/touch.rs` |
   | `grid_tests.rs` | `src/room/components/participant_grid/tests.rs` |
   | `MemberCardSignals` (bundle) | one flat `RoomState` struct in `src/room/state.rs`; `is_touch` / `controls_visible` / `expanded` are fields on it, delivered via `provide_context` |
   | `public/styles/card.css` (touch quality/volume rules) | `public/styles/card-widgets.css` `@media (hover: none) and (pointer: coarse)` block |

   A handful of doc-comments still name `member_card.rs` / `grid.rs` (e.g.
   `media_controls.rs:2`, `dev_preview.rs:186`); fixing those is
   out of scope here but fair game to sweep while editing a file.

## How the findings were reproduced

Headless Chromium driven by Playwright, `mobile-web`-style context
(`viewport 390×844`, `hasTouch: true`, `isMobile: true`, iOS UA), a real
two-context room (sharer + phone viewer) with the fake-media Chrome flags.
`pointer: coarse` / `@media (hover: none) and (pointer: coarse)` only
activates with a real `hasTouch` context — plain viewport resizing does
**not** trigger it, so the touch layout has to be tested through that
project, not by resizing.

Not covered by automation (verify by hand on a real device before closing
the PR): real screen capture, audio, bitrate adaptation, the browser's own
"stop sharing" control, iOS Safari fullscreen behaviour,
`env(safe-area-inset-*)` with a real notch / home indicator.

## Findings, by severity

| ID | Sev | Status vs. current code | Summary |
|----|-----|------------------------|---------|
| A1 | High | **open** | Quality bottom-sheet options can sit **behind** the floating control bar → unselectable on a phone. The touch `.quality-menu__popup` (now in `card-widgets.css`) is `position: fixed` with **no `z-index`**; `.room-controls` has `z-index: 10` and is visible whenever the sheet is (the sheet only opens in focus mode with the chrome shown). |
| A2 | Low | **mostly fixed** | The per-stream volume popover is `display: none` on touch (ADR-0007); only the mute toggle remains, in the card-action row. Residual: confirm that mute toggle doesn't paint over the open sheet. |
| A3 | Med | **open** | The fixed `.room-controls` bar overlaps (a) the focus-mode filmstrip's bottom row and (b) — now that the roster-view bar never auto-hides on touch — the last row of roster cards, permanently. Nothing reserves space for the bar. |
| A4 | High | **open** | Fullscreen on touch is a trap. `card_click` (`participant/mod.rs`) still starts with `if exit_fullscreen_if_active() { return; }`, so every tap on a fullscreen card only exits fullscreen. `setup_fullscreen_autohide_controls` (`media_controls.rs`) still re-reveals the idle controls **only on `mousemove`**, which touch never fires. |
| A5 | Med | **open** | `toggle_fullscreen` (`media_controls.rs`) calls `card.request_fullscreen()` on the `.card` `<div>`; iOS Safari rejects that (only `<video>` may go fullscreen), so the "Tela cheia" button is a silent no-op on iPhone. |
| B1 | Med | **open** | Focused video does not fill the width in portrait. `.card--focus { grid-column: 1 / -1 }` spans only the filmstrip tracks `auto-fit` actually materialised (`repeat(auto-fit, minmax(4.5rem, 6rem))` + `justify-content: center` on touch), so with few members the focused card is a fraction of the viewport with dead bars each side. The CSS comment still says "a true sideways tray needs a wrapper element this CSS-only pass doesn't add." |
| B2 | Med | **open** | Focus-mode filmstrip tiles (~96×68 px on touch: `minmax(4.5rem, 6rem)` cols, `4.25rem` rows) are too cramped: the 2.5rem avatar, the nick pill, and the (scaled-down but still rendered) `.card__corner-start` ping/"você" cluster overlap. |
| C1 | Low | **likely fixed; verify** | `recompute_adaptive_grid` already computes a `center_offset` for a sparse last row, and `best_column_count` forces ≥2 columns under 560px with 3+ members (both unit-tested in `participant_grid/tests.rs`). A 3-member narrow layout traces to `grid-column: 2 / span 2` (centred). The one remaining risk: `recompute_adaptive_grid` bails out with `if width <= 0.0 || height <= 0.0 { return; }` and only re-runs on a tracked signal change or the window `resize` event — there is **no `ResizeObserver`** and no retry, so a first-frame zero measurement on a phone can leave the grid un-centred until the next state change. |
| C2 | Low | **open** | "Assistir transmissão" truncates to "Assistir tra…" on the narrow 2-column phone roster grid. `.card__watch-pill` is `white-space: nowrap; text-overflow: ellipsis` with no narrow-width override (only the filmstrip context shrinks its font). |

Scenario "tap on the expanded card exits focus mode" from the original
manual report **could not be reproduced** (focus survives every tap
variant — `card_click` only toggles `controls_visible` in that case).
Confirmed not happening. Not in scope.

---

## Phase 1 — Controls the user cannot reach (A1, A2, A3, A4, A5)

Highest impact: on a phone today you cannot reliably pick a lower video
quality, and you cannot get back to the controls once you are in
fullscreen.

### 1.1 Quality bottom-sheet stacking (A1, A2)

**Where:**
- `apps/web/public/styles/card-widgets.css` — the
  `@media (hover: none) and (pointer: coarse)` block. `.quality-menu__popup`
  becomes `position: fixed; inset: auto 0 0 0` with **no `z-index`**;
  opens on `:focus-within`.
- `apps/web/public/styles/room.css` — `.room-controls { z-index: 10 }`
  (around line 195).
- `apps/web/src/room/components/participant/parts.rs` — `QualityMenu`
  (the trigger's `mousedown`/`click` open/close-on-second-tap logic).

**Root cause:** the sheet and `.room-controls` are separate `position: fixed`
layers; `position: fixed` establishes a stacking context at `z-index: auto`
(level 0), so `.room-controls` at `z-index: 10` paints over the sheet's
lower rows. The sheet only opens in focus mode, and the focused card's
actions are only shown while the chrome is up (`.room-page:not(.chrome-hidden)`),
which is exactly when `.room-controls` is visible — so the collision is
the normal case, not an edge one.

**Fix:**
- [ ] Give the touch `.quality-menu__popup` an explicit `z-index` above
      `.room-controls` (e.g. `z-index: 50`). Add a short comment naming the
      `.room-controls` value it has to beat.
- [ ] While the sheet is open, take the control bar out of the way:
      `.room-page:has(.quality-menu:focus-within) .room-controls { opacity: 0; pointer-events: none; }`
      (`:has()` is supported on current mobile Safari/Chrome). Belt and
      braces on top of the `z-index` bump, and it also stops the bar
      intercepting taps aimed just above the sheet.
- [ ] Add a dim full-screen scrim behind the sheet on touch (a
      `.quality-menu__popup::before { position: fixed; inset: 0; }` pinned
      below the sheet, or a real backdrop element) so a tap outside is an
      obvious "close" target and the options read against a calm ground.
      The existing "tap the header to close" behaviour must keep working.
- [ ] A2: confirm the residual mute toggle (`.volume-control` → its
      `.icon-btn` on touch) never paints over the open sheet. The
      `z-index` bump on the popup should already win; assert it rather than
      assume it. `.volume-control__popup` stays `display: none` on touch
      (unchanged).

### 1.2 Floating bar overlaps the filmstrip and the last card row (A3)

**Where:** `apps/web/public/styles/room.css` — `.room-controls`
(`position: fixed; bottom: calc(1.5rem + env(safe-area-inset-bottom))`),
the `.grid` / `.grid--focused` padding, and the
`@media (hover: none) and (pointer: coarse)` `.grid--focused` override.
The touch auto-hide behaviour is in
`apps/web/src/room/components/participant_grid/mod.rs`
(`setup_auto_hide_controls`): in focus mode the bar arms the fade; in the
roster view on touch it is pinned visible (`controls_visible.set(true)`,
`cancel_pending()`).

**Root cause:** nothing reserves vertical space for the bar. The
focus-mode filmstrip's bottom row renders under it, and — because the
roster-view bar no longer fades on touch — so does the last row of roster
cards, permanently.

**Fix:**
- [ ] Introduce one shared CSS custom property for the bar's reserved
      height (bar button height + `1.5rem` + `env(safe-area-inset-bottom)`),
      defined once (e.g. on `.room-page`) with a one-line comment, instead
      of repeating the literal.
- [ ] On touch, add `padding-bottom: var(--controls-reserve)` to
      **`.grid`** (roster) and to **`.grid--focused`** (focus mode) so the
      filmstrip tray and the last roster row both end above the bar. The
      grid already has `overflow-y: auto`, so the padding just extends the
      scroll area.
- [ ] Verify the empty-room and 1-member cases don't gain an ugly dead
      strip — the padding is only a floor for when content reaches the
      bottom.

### 1.3 Fullscreen on touch is a trap (A4, A5)

**Where:**
- `apps/web/src/room/components/participant/mod.rs` — `card_click` starts
  with `if exit_fullscreen_if_active() { return; }`, so **every** tap on a
  fullscreen card only exits fullscreen.
- `apps/web/src/room/components/participant/action_bar.rs` — renders the
  "Tela cheia" and "Picture-in-picture" `.icon-btn`s, gated only on
  `showing_video()`.
- `apps/web/src/room/media_controls.rs` —
  `setup_fullscreen_autohide_controls` re-reveals the idle controls **only
  on `mousemove`** (never fired by touch), and `toggle_fullscreen` calls
  `card.request_fullscreen()` on a `<div>` (iOS rejects it → A5).

**Reproduced (A4):** enter focus → tap "Tela cheia"
(`document.fullscreenElement` set) → wait ~3.5 s
(`card--controls-idle` added) → tap the card → `document.fullscreenElement`
becomes `null`, controls unreachable.

**Recommended fix — drop OS fullscreen on touch; focus mode *is* the
phone's "maximized" view.** Focus mode already fills the viewport minus
the header, already has tap-to-toggle chrome (`controls_visible`), already
has a "Voltar para a grade" exit in the bar, and already works on iOS
(where real `.card` fullscreen never will). ADR-0007 already frames focus
mode as the mobile equivalent — this just removes the dead entry point.

- [ ] In `action_bar.rs`, gate the "Tela cheia" and "Picture-in-picture"
      buttons on `!is_touch.get()` (the `is_touch` signal is a field of
      `RoomState`, read it from context). That removes the only touch
      entry point into `toggle_fullscreen`, resolving A4 (nothing to get
      stuck in) and A5 (no silent no-op).
- [ ] Leave `card_click`'s `exit_fullscreen_if_active()` guard in place —
      it is a harmless no-op on touch once the buttons are gone
      (`document.fullscreenElement` stays `null`), and it still matters on
      desktop.
- [ ] `setup_fullscreen_autohide_controls` can stay as-is (desktop-only in
      practice now). Optionally add a one-line comment that its `mousemove`
      basis is intentional — touch has no fullscreen to autohide for.
- [ ] Record the decision: update ADR-0007's "The mobile room (touch
      only)" section (or add a short ADR-0010) stating OS fullscreen /
      Picture-in-picture is disabled on touch, focus mode replaces it.

**Smaller alternative (only if the maintainer wants to keep Android Chrome
fullscreen, where `.card` fullscreen does work on touch):** drive
`card--controls-idle` from an `RwSignal<bool>` on `RoomState` (same
pattern as `controls_visible`), and in `card_click`: if fullscreen +
touch + controls currently idle → clear idle and `return` *without*
exiting; only exit when the controls are already visible or on non-touch.
Still needs the `is_touch` gate for A5 on iOS. More code, more risk — the
recommended fix is preferred.

### Phase 1 tests (`apps/web/end2end/tests/room-mobile.spec.ts`, add cases)

- [ ] With the quality sheet open, every option is `toBeInViewport`
      **and** hit-testable — `elementFromPoint` at each option's centre
      returns that option, not `.control-group` / `.icon-btn` /
      `.volume-control`.
- [ ] With the sheet open, `.room-controls` is not intercepting (opacity 0
      / `pointer-events: none` / out of the hit-test).
- [ ] Focus mode: `.grid--focused` filmstrip bottom ≤ `.room-controls`
      top (or `.grid--focused` computed `padding-bottom` ≥ the reserved
      height var).
- [ ] Roster view on touch: the last visible `.card`'s bottom ≤
      `.room-controls` top.
- [ ] Touch: "Tela cheia" / "Picture-in-picture" buttons are not present
      on a card, and `document.fullscreenElement` stays `null` through a
      patch → focus → wait-for-idle → tap-to-reveal flow.

---

## Phase 2 — Focus-mode layout on touch (B1, B2)

### 2.1 Focused video does not fill the width (B1)

**Where:** `apps/web/public/styles/room.css` — `.grid--focused` and its
touch override
(`grid-template-columns: repeat(auto-fit, minmax(4.5rem, 6rem))` +
`justify-content: center`); `apps/web/public/styles/card.css` —
`.card--focus { grid-column: 1 / -1; grid-row: 1 }`. `1 / -1` only spans
the filmstrip tracks that materialised, and with few members that is a
fraction of the width; `justify-content: center` then splits the leftover
into dead bars each side.

**Fix (CSS-only, contained):**
- [ ] On touch, add `position: relative` to `.grid--focused` and take the
      focused card out of grid flow:
      `.grid--focused .card--focus { position: absolute; inset: 0 0 calc(var(--filmstrip-row) + <gap>) 0; }`
      so the focused video is full-bleed regardless of how many filmstrip
      columns exist. The filmstrip grid keeps its row-2 layout underneath.
      Use a CSS var for the filmstrip row height (`4.25rem` on touch)
      instead of a literal.
- [ ] Verify landscape (where the card already spans the full width) is
      unchanged or improved.

**Proper alternative (larger, schedule separately if the CSS-only fix
proves fragile):** wrap the non-focused cards in a real `.filmstrip`
element (`display: flex; overflow-x: auto`) and make `.grid--focused` a
`flex-direction: column` with the focused area `flex: 1`. Needs a DOM
change in `member_cards()` / the `#member-grid` markup
(`participant/mod.rs`, `components/stage.rs`) and a rethink of the
`MAX_MEMBERS` flat-slot-list assumption.

### 2.2 Filmstrip tiles are cramped (B2)

**Where:** `apps/web/public/styles/card.css` —
`.grid--focused .card:not(.card--focus)` overrides (touch
`grid-auto-rows: 4.25rem`, tile `minmax(4.5rem, 6rem)`; avatar forced to
`2.5rem`; `.card__corner-start` scaled but still rendered).

**Fix:**
- [ ] Hide `.card__corner-start` (ping badge + "você" tag) inside
      non-focused filmstrip tiles on touch — unreadable at that size, pure
      clutter.
- [ ] Shrink the filmstrip avatar to ~2rem on touch and pin the nick pill
      to the very bottom (`bottom: 2px`, smaller padding/font) so they no
      longer overlap; or drop the nick pill and rely on the avatar
      letter + colour. Pick one, keep it consistent with 2.1's sizing.
- [ ] Ensure `.card__nick` in the tile ellipsises cleanly rather than
      clipping mid-glyph.

### Phase 2 tests

- [ ] Portrait focus mode: `.card--focus` width ≥ 0.98 × viewport width.
- [ ] Each filmstrip tile: no child's bounding box exceeds the tile's box;
      avatar and nick pill do not vertically overlap;
      `.card__corner-start` is not rendered.
- [ ] Landscape focus mode: `.card--focus` still spans the full width; no
      regression.

---

## Phase 3 — Grid polish (C1, C2)

### 3.1 Orphan last-row card centring on touch (C1)

**Where:** `apps/web/src/room/components/participant_grid/mod.rs` —
`recompute_adaptive_grid` (the `center_offset` math) and `setup_adaptive_grid`
(the `Effect` + window `resize` listener that drive it).
`apps/web/src/room/components/participant_grid/tests.rs` — the
`best_column_count` unit tests.

**Status:** the centring math and the ≥2-column floor are already in place
and unit-tested; a 3-member narrow layout traces to a centred
`grid-column`. Treat this phase as **verify first**:

- [ ] Add a `mobile-web` Playwright case: 3 members, narrow touch
      viewport, roster view — the last card's computed `grid-column` start
      is `> 1`. If it passes, C1 is closed; document that and stop.
- [ ] Only if it fails: the likely cause is `recompute_adaptive_grid`
      reading `client_width` / `client_height` as `0` on the first
      `requestAnimationFrame` and never recomputing. Fix by re-running on
      the first non-zero `ResizeObserver` tick for `#member-grid` (add a
      `ResizeObserver` alongside the existing `resize` listener, torn down
      via `listen_until_cleanup`'s equivalent), and/or a bounded retry
      when the measured size is `0`. Extend the native tests only if the
      offset math itself changes.

### 3.2 "Assistir transmissão" truncated (C2)

**Where:** `apps/web/public/styles/card.css` — `.card__watch-pill`
(`white-space: nowrap; text-overflow: ellipsis`).

**Fix:**
- [ ] In the roster (non-focused) context on narrow widths, let the pill
      wrap to 2 lines (`white-space: normal`, `-webkit-line-clamp: 2`,
      tight `line-height`) or drop its font-size so "Assistir transmissão"
      fits. Keep `nowrap` where the tile is wide enough (focused card,
      desktop) and keep the filmstrip-tile shrink rule intact.

### Phase 3 tests

- [ ] 3 members, narrow touch viewport: last card's computed
      `grid-column` start `> 1`.
- [ ] `.card__watch-pill` in a 2-column phone roster grid:
      `scrollWidth <= clientWidth` (not truncated) or it renders on 2
      lines with no clipping.

---

## Definition of done (every phase)

- [ ] `scripts/test-all.sh --no-mutants` green (fmt, clippy native +
      wasm, `cargo leptos build`, `cargo test --workspace --features ssr`,
      the `hydrate` WASM suite, Playwright `desktop` + `mobile-web`).
- [ ] New Playwright assertions above pass under the `mobile-web` project;
      the `desktop` project is unchanged.
- [ ] Hand-verified on a real iPhone (Safari) **and** a real Android
      (Chrome): quality sheet fully reachable, no fullscreen trap, focused
      video fills the width, filmstrip legible, controls never permanently
      hidden behind the bar, last roster row not under the bar.
- [ ] No desktop regression — `cargo leptos watch`, room with 2 and 4
      members, hover controls, quality/volume menus, control-bar
      auto-hide, and desktop fullscreen + PiP all unchanged.
- [ ] Docs updated when an invariant moved: ADR-0007 (or a new ADR-0010)
      records that OS fullscreen / PiP is disabled on touch and focus mode
      replaces it. `docs/architecture/*` stays unchanged (presentation
      layer only).
