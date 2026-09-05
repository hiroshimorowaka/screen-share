# 0009 — Per-surface audio for browser screen sharing

Status: proposed (not yet implemented)
Date: 2026-09-03
Revised: 2026-09-04 — paths/API notes re-based onto the 8-phase structure
refactor (`f63cbcb`); the decision itself is unchanged and still unbuilt.

## Context

Today a plain browser share asks `getDisplayMedia({ video: true, audio: true })`.
The constraints are built in
`apps/web/src/client/webrtc/screen_share.rs` — `display_media_constraints`,
called from `capture_display` in the same file. Chrome then only ever puts
an audio track on the stream when the shared surface is a **tab** and the
user ticks "share tab audio". Sharing a window or the whole screen yields
video only. The desktop shell keeps capturing audio through its own
platform backend (`is_desktop_app()` branch in `capture_display`) and
still requests video only.

We want the browser share to carry the audio that matches the chosen
surface:

| Surface (`displaySurface`) | Audio it should carry            |
| -------------------------- | -------------------------------- |
| tab (`browser`)            | that tab's audio — unchanged     |
| window (`window`)          | only that window's own audio     |
| full screen (`monitor`)    | the whole system's audio         |

And in every case the capture must never fold in audio produced by our own
tab (the room page itself), so watchers are not echoed back.

## Decision

### Constraints passed to `getDisplayMedia` (browser capture only)

```js
{
  video: true,
  audio: { restrictOwnAudio: true },  // ALWAYS on — never capture our own tab's audio
  systemAudio: "include",             // full screen -> offer system audio
  windowAudio: "window",              // window      -> offer only that window's audio
  // tab audio needs no hint; Chrome pre-selects it for the `browser` surface
}
```

- **`restrictOwnAudio: true` is unconditional.** Set on every browser
  capture regardless of surface. It is a hint (reflected back on
  `MediaTrackSettings.restrictOwnAudio`), so we also keep the existing
  guard that a sharer never opens a peer connection to themselves — this
  is defence in depth, not the only line.
- **`systemAudio: "include"`** makes Chrome offer the system-audio toggle
  when the user picks a monitor. The OS can still refuse (Linux has no
  path; macOS needs 14.2+ / Chrome 141+; Windows and ChromeOS work).
- **`windowAudio: "window"`** (Chrome 141+) asks that a window share carry
  only that window's audio, not the whole system. Older Chrome ignores it
  and simply gives no window audio, which is the current behaviour — safe
  degradation.
- We do **not** set `systemAudio: "exclude"`. Excluding is the opposite of
  what we want for the monitor surface.

### Desktop shell

Unchanged. The `is_desktop_app()` branch in `capture_display` still
requests `{ video: true }` only; system audio comes from the Electron
platform backend (`docs/decisions/0004-desktop-electron-and-windows-native-audio.md`).
`restrictOwnAudio` is irrelevant there because no audio track comes from
`getDisplayMedia`.

### Implementation notes

- `display_media_constraints(desktop: bool) -> DisplayMediaStreamConstraints`
  currently does `set_video_bool(true)` and, for `!desktop`,
  `set_audio_bool(true)` on the immutable-builder object (web-sys
  `0.3.104`). Replace the plain `set_audio_bool(true)` on the browser
  branch with an `audio` **constraints object** carrying
  `restrictOwnAudio: true`, and set `systemAudio` / `windowAudio` on the
  top-level constraints.
- web-sys `0.3.104` has no typed setter for `restrictOwnAudio`,
  `systemAudio`, or `windowAudio`. Build a plain `js_sys::Object`, set the
  keys with `js_sys::Reflect::set`, and hand it to `set_audio(&audio_obj)`
  / `Reflect::set` on the constraints — the same non-typed-constraint
  pattern already used elsewhere in this module. (`MediaTrackConstraints`
  is an enabled feature but exposes none of these three, so a typed object
  buys nothing here.)
- The returned stream may still have **no audio track** (user unticked the
  box, OS refused, unsupported browser). Downstream already treats the
  audio track as optional — `video_and_audio_tracks` in
  `apps/web/src/room/media/mod.rs` returns `Option`s and
  `replace_track(null)` clears an absent audio sender. Keep that.
- No wire-protocol or signaling change: this is entirely inside
  `display_media_constraints` / `capture_display`.

## Consequences

- Firefox/Safari: still no capture audio at all (they ignore the audio
  part of `getDisplayMedia`). No regression.
- Linux Chrome: tab audio still works; window/monitor audio still absent
  because the OS has no path — the hints are harmless no-ops.
- One extra thing to hand-verify per the "Browser layer" definition of
  done: window capture carries only that window's sound, full-screen
  capture carries system sound, and neither carries the room page's own
  audio.

## Tests

- `wasm-bindgen-test` in
  `apps/web/src/client/webrtc/screen_share_wasm_tests.rs` (which already
  covers `display_media_constraints`): extend the browser-branch test to
  assert `audio` is an object with `restrictOwnAudio === true`,
  `systemAudio === "include"`, `windowAudio === "window"`, and that the
  desktop-branch test still sees video only with no audio constraint.
- Real system/window audio routing stays hand-verified (no automation for
  OS audio loopback), noted in the change.
