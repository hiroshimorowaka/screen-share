# Desktop System Audio Sharing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user of the Electron desktop app optionally share their
system's whole audio output alongside their screen — opt-in via a
checkbox that only exists in the desktop app — with no changes needed
on the receiving end (desktop or plain browser), since the existing
per-viewer volume control already works on any audio track it finds.

**Architecture:** The Electron main process spawns/kills a `pw-loopback`
subprocess (already installed as part of PipeWire) that exposes system
audio as a normal-looking microphone device. The web app (same Rust/
Leptos code everywhere) detects it's running inside the desktop app via
a `window.desktopAudio` bridge the main process's preload script
injects, and when audio sharing is requested, grabs that virtual
device via ordinary `getUserMedia` and merges its audio track into the
same `MediaStream` used for the screen video — before the first WebRTC
offer is ever created, so no signaling protocol changes are needed.

**Tech Stack:** Electron/TypeScript (`desktop/`, already set up),
`web-sys`/`js-sys`/`wasm-bindgen` (already set up, three new web-sys
features needed), the system's own `pw-loopback` and `pw-dump` CLI
tools (no new Rust crate).

## Global Constraints

- No new Rust crate for this plan — that's item 3 (process exclusion), not this one.
- The audio-share checkbox must never appear or do anything outside the desktop app (i.e. `window.desktopAudio` absent ⇒ hidden, and `share_audio` can never be `true`).
- Stopping a share that had audio active must always kill the `pw-loopback` process — including via the browser's native "Stop sharing" control and via the tray's "Sair".
- No signaling/protocol changes — the combined `MediaStream` must exist before the first `RTCPeerConnection` offer is created.
- Checkbox defaults to unchecked every time (no persistence across sessions).
- Linux only, matches the rest of the desktop app's scope so far.

---

### Task 1: Electron main process manages the audio loopback device

**Files:**
- Modify: `desktop/src/main.ts`
- Modify: `desktop/src/preload.ts`

**Interfaces:**
- Produces: `window.desktopAudio.start(): Promise<void>` (rejects if the
  device doesn't appear within 3 seconds) and
  `window.desktopAudio.stop(): Promise<void>`, available in every
  window that uses `preload.js` — which, after this task, includes the
  main window (today only the picker window has a preload). Task 2
  calls these from the Rust/WASM side.

- [ ] **Step 1: Add the IPC handlers to `desktop/src/main.ts`**

Add this import to the existing import line:

```typescript
import { app, BrowserWindow, Tray, Menu, session, desktopCapturer, ipcMain } from 'electron';
```

becomes:

```typescript
import { app, BrowserWindow, Tray, Menu, session, desktopCapturer, ipcMain } from 'electron';
import { spawn, ChildProcess } from 'child_process';
```

Add this block right after the `app.on('before-quit', ...)` block
(after line 52, before the `interface PickerSource` block):

```typescript
let audioLoopback: ChildProcess | null = null;

function stopAudioLoopback(): void {
  if (audioLoopback) {
    audioLoopback.kill();
    audioLoopback = null;
  }
}

function isLoopbackDevicePresent(): Promise<boolean> {
  return new Promise((resolve) => {
    const dump = spawn('pw-dump');
    let output = '';
    dump.stdout.on('data', (chunk: Buffer) => {
      output += chunk.toString();
    });
    dump.on('close', () => {
      resolve(output.includes('"node.name": "screen_share_audio"'));
    });
    dump.on('error', () => resolve(false));
  });
}

async function waitForLoopbackDevice(timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await isLoopbackDevicePresent()) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error('Timed out waiting for the audio loopback device to appear');
}

ipcMain.handle('start-audio-loopback', async () => {
  if (audioLoopback) return;
  // `media.class=Audio/Source` must be on the *playback* side, not the
  // capture side — an earlier version of this had them backwards,
  // producing a correctly-named, selectable device that carried pure
  // silence (confirmed by recording it directly with pw-record).
  audioLoopback = spawn('pw-loopback', [
    '-C', '@DEFAULT_SINK@',
    '--capture-props', 'stream.capture.sink=true node.passive=true',
    '--playback-props', 'media.class=Audio/Source node.name=screen_share_audio node.description="Screen Share Audio"',
  ]);
  audioLoopback.on('exit', () => {
    audioLoopback = null;
  });
  try {
    await waitForLoopbackDevice(3000);
  } catch (err) {
    stopAudioLoopback();
    throw err;
  }
});

ipcMain.handle('stop-audio-loopback', () => {
  stopAudioLoopback();
});
```

Add `stopAudioLoopback();` as the first line inside the existing
`app.on('before-quit', () => { ... })` block, so it reads:

```typescript
app.on('before-quit', () => {
  stopAudioLoopback();
  isQuitting = true;
});
```

- [ ] **Step 2: Give the main window a preload script**

In `createMainWindow()`, change:

```typescript
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
  });
```

to:

```typescript
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
    },
  });
```

- [ ] **Step 3: Expose `window.desktopAudio` from the preload script**

Add this to `desktop/src/preload.ts`, after the existing
`contextBridge.exposeInMainWorld('picker', ...)` block:

```typescript
contextBridge.exposeInMainWorld('desktopAudio', {
  start: () => ipcRenderer.invoke('start-audio-loopback'),
  stop: () => ipcRenderer.invoke('stop-audio-loopback'),
});
```

- [ ] **Step 4: Compile**

```bash
cd desktop && pnpm exec tsc
```

Expected: no errors.

- [ ] **Step 5: Run it and verify manually**

```bash
pnpm exec electron .
```

Open the main window's DevTools (`Ctrl+Shift+I`) and in the console run:

```js
await window.desktopAudio.start()
```

Expected: the Promise resolves (no error). In another terminal, confirm
the device exists:

```bash
wpctl status
```

Expected: a Source named `screen_share_audio` (or `pw-loopback-<pid>`
with a "Screen Share Audio" description) appears under `Sources:`.

Now in the DevTools console:

```js
await window.desktopAudio.stop()
```

Expected: `wpctl status` no longer lists it.

Test cleanup-on-quit: run `await window.desktopAudio.start()` again,
then quit the app from the tray ("Sair") without calling `stop()`
first. Expected: `wpctl status` shows no lingering `screen_share_audio`
device afterward.

- [ ] **Step 6: Commit**

```bash
git add desktop/src/main.ts desktop/src/preload.ts
git commit -m "feat(desktop): manage a pw-loopback system-audio device over IPC"
```

---

### Task 2: Web app opt-in audio sharing

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/ui/client/webrtc.rs`
- Modify: `src/ui/pages/room/share.rs`
- Modify: `src/ui/pages/room/mod.rs`

**Interfaces:**
- Consumes: `window.desktopAudio.start()` / `.stop()` from Task 1.
- Produces: `capture_display(share_audio: bool) -> Result<MediaStream, JsValue>` (replaces the current zero-argument `capture_display()`), `is_desktop_app() -> bool` in `webrtc.rs`; `desktop_audio_supported() -> bool` in `share.rs`.

- [ ] **Step 1: Add the three new web-sys features to `Cargo.toml`**

In the `web-sys` features list, add `"MediaDeviceInfo"`,
`"MediaDeviceKind"`, `"MediaTrackConstraints"`, and
`"MediaStreamConstraints"` — e.g. right after `"DisplayMediaStreamConstraints",`:

```toml
    "DisplayMediaStreamConstraints",
    "MediaDeviceInfo",
    "MediaDeviceKind",
    "MediaTrackConstraints",
    "MediaStreamConstraints",
```

- [ ] **Step 2: Add `is_desktop_app()` to `src/ui/client/webrtc.rs`**

Add near `is_display_media_supported()` (end of the file):

```rust
pub fn is_desktop_app() -> bool {
    let Some(window) = web_sys::window() else { return false };
    js_sys::Reflect::has(&window, &JsValue::from_str("desktopAudio")).unwrap_or(false)
}
```

- [ ] **Step 3: Replace `capture_display()` in `src/ui/client/webrtc.rs`**

Replace the existing `capture_display` function:

```rust
pub async fn capture_display() -> Result<MediaStream, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window: not running in a browser"))?;
    let media_devices = window.navigator().media_devices()?;

    let constraints = DisplayMediaStreamConstraints::new();
    constraints.set_video_bool(true);

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(promise).await?;
    stream.dyn_into::<MediaStream>()
}
```

with:

```rust
pub async fn capture_display(share_audio: bool) -> Result<MediaStream, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window: not running in a browser"))?;
    let media_devices = window.navigator().media_devices()?;

    let constraints = DisplayMediaStreamConstraints::new();
    constraints.set_video_bool(true);

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(promise).await?;
    let video_stream = stream.dyn_into::<MediaStream>()?;

    if !share_audio {
        return Ok(video_stream);
    }

    start_desktop_audio_loopback().await?;
    match capture_loopback_audio(&media_devices).await {
        Ok(audio_stream) => combine_video_and_audio(&video_stream, &audio_stream),
        Err(err) => {
            let _ = stop_desktop_audio_loopback().await;
            Err(err)
        }
    }
}

async fn start_desktop_audio_loopback() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let desktop_audio = js_sys::Reflect::get(&window, &JsValue::from_str("desktopAudio"))?;
    let start_fn: js_sys::Function =
        js_sys::Reflect::get(&desktop_audio, &JsValue::from_str("start"))?.dyn_into()?;
    let promise: js_sys::Promise = start_fn.call0(&desktop_audio)?.dyn_into()?;
    JsFuture::from(promise).await?;
    Ok(())
}

pub async fn stop_desktop_audio_loopback() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let desktop_audio = js_sys::Reflect::get(&window, &JsValue::from_str("desktopAudio"))?;
    let stop_fn: js_sys::Function =
        js_sys::Reflect::get(&desktop_audio, &JsValue::from_str("stop"))?.dyn_into()?;
    let promise: js_sys::Promise = stop_fn.call0(&desktop_audio)?.dyn_into()?;
    JsFuture::from(promise).await?;
    Ok(())
}

async fn capture_loopback_audio(media_devices: &web_sys::MediaDevices) -> Result<MediaStream, JsValue> {
    let promise = media_devices.enumerate_devices()?;
    let devices: js_sys::Array = JsFuture::from(promise).await?.dyn_into()?;

    let mut device_id = None;
    for device in devices.iter() {
        let info: web_sys::MediaDeviceInfo = device.dyn_into()?;
        if info.kind() == web_sys::MediaDeviceKind::Audioinput
            && info.label().contains("Screen Share Audio")
        {
            device_id = Some(info.device_id());
            break;
        }
    }
    let device_id = device_id
        .ok_or_else(|| JsValue::from_str("Screen Share Audio device not found"))?;

    let track_constraints = MediaTrackConstraints::new();
    track_constraints.set_device_id_str(&device_id);
    let audio_constraints = MediaStreamConstraints::new();
    audio_constraints.set_audio_media_track_constraints(&track_constraints);

    let promise = media_devices.get_user_media_with_constraints(&audio_constraints)?;
    JsFuture::from(promise).await?.dyn_into::<MediaStream>()
}

fn combine_video_and_audio(video: &MediaStream, audio: &MediaStream) -> Result<MediaStream, JsValue> {
    let tracks = js_sys::Array::new();
    for track in video.get_tracks().iter() {
        tracks.push(&track);
    }
    for track in audio.get_tracks().iter() {
        tracks.push(&track);
    }
    MediaStream::new_with_tracks(&tracks)
}
```

Add `MediaTrackConstraints` and `MediaStreamConstraints` to the
existing `web_sys::{...}` import list at the top of the file, so it
reads:

```rust
use web_sys::{
    DisplayMediaStreamConstraints, MediaStream, MediaStreamConstraints, MediaTrackConstraints,
    RtcConfiguration, RtcIceCandidateInit, RtcIceServer, RtcPeerConnection, RtcSdpType,
    RtcSessionDescriptionInit,
};
```

- [ ] **Step 4: Run `cargo check` for both targets**

```bash
cargo check --features ssr
cargo check --target wasm32-unknown-unknown --features hydrate
```

Expected: both succeed. (`capture_display` now takes an argument, so
this step will also surface the one call site that needs updating —
fixed in the next step.)

- [ ] **Step 5: Add `desktop_audio_supported()` and thread `share_audio` through `src/ui/pages/room/share.rs`**

Add this function near `share_supported()`:

```rust
#[cfg(not(feature = "hydrate"))]
pub(super) fn desktop_audio_supported() -> bool {
    false
}

#[cfg(feature = "hydrate")]
pub(super) fn desktop_audio_supported() -> bool {
    crate::ui::client::webrtc::is_desktop_app()
}
```

Change `share_toggle_handler`'s signature (both the `not(hydrate)` stub
and the `hydrate` version) to take two more parameters,
`share_audio: ReadSignal<bool>` and `sharing_with_audio: RwSignal<bool>`,
right after `own_preview_hidden`:

```rust
#[cfg(not(feature = "hydrate"))]
pub(super) fn share_toggle_handler(
    _conn: RoomConnection,
    _is_sharing: ReadSignal<bool>,
    _set_is_sharing: WriteSignal<bool>,
    _own_preview_hidden: RwSignal<bool>,
    _share_audio: ReadSignal<bool>,
    _sharing_with_audio: RwSignal<bool>,
    _set_status: WriteSignal<String>,
    _my_peer_id: ReadSignal<Option<String>>,
    _expanded: RwSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn share_toggle_handler(
    conn: RoomConnection,
    is_sharing: ReadSignal<bool>,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    share_audio: ReadSignal<bool>,
    sharing_with_audio: RwSignal<bool>,
    set_status: WriteSignal<String>,
    my_peer_id: ReadSignal<Option<String>>,
    expanded: RwSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
```

Inside the `hydrate` version, change:

```rust
    move |_| {
        if is_sharing.get_untracked() {
            stop_sharing(&conn, set_is_sharing, own_preview_hidden, expanded, my_peer_id);
            return;
        }

        let conn = conn.clone();
        let my_peer_id_value = my_peer_id.get_untracked();
        set_status.set("Selecione a tela para compartilhar...".to_string());

        spawn_local(async move {
            let stream = match capture_display().await {
```

to:

```rust
    move |_| {
        if is_sharing.get_untracked() {
            stop_sharing(&conn, set_is_sharing, own_preview_hidden, sharing_with_audio, expanded, my_peer_id);
            return;
        }

        let conn = conn.clone();
        let my_peer_id_value = my_peer_id.get_untracked();
        let share_audio_value = share_audio.get_untracked();
        set_status.set("Selecione a tela para compartilhar...".to_string());

        spawn_local(async move {
            let stream = match capture_display(share_audio_value).await {
```

A few lines down, right after `set_is_sharing.set(true);`, add:

```rust
            sharing_with_audio.set(share_audio_value);
```

And inside the `onended` closure (the native "Stop sharing" button
path), change:

```rust
                let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                    stop_sharing(&conn_for_end, set_is_sharing, own_preview_hidden, expanded, my_peer_id);
                });
```

to:

```rust
                let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                    stop_sharing(&conn_for_end, set_is_sharing, own_preview_hidden, sharing_with_audio, expanded, my_peer_id);
                });
```

Now update `stop_sharing` itself — add `sharing_with_audio: RwSignal<bool>`
as a parameter (right after `own_preview_hidden`) and, at the very
start of the function body, stop the loopback if this share had audio:

```rust
#[cfg(feature = "hydrate")]
pub(super) fn stop_sharing(
    conn: &RoomConnection,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    sharing_with_audio: RwSignal<bool>,
    expanded: RwSignal<Option<String>>,
    my_peer_id: ReadSignal<Option<String>>,
) {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;

    if sharing_with_audio.get_untracked() {
        sharing_with_audio.set(false);
        spawn_local(async {
            let _ = crate::ui::client::webrtc::stop_desktop_audio_loopback().await;
        });
    }

    if let Some(stream) = conn.local_stream.borrow_mut().take() {
```

(`stop_desktop_audio_loopback` is already `pub` from Step 3, so
`stop_sharing` — in a different module — can call it directly; no
extra wrapper needed.)

`leave_or_stop_watching_handler` in `watch.rs` also calls into share
state indirectly through `expanded`/`watching` but does **not** call
`stop_sharing` — sharing and watching are independent, so it needs no
change.

- [ ] **Step 6: Wire the checkbox and new signals into `src/ui/pages/room/mod.rs`**

Add a new signal next to `own_preview_hidden`:

```rust
    let own_preview_hidden = RwSignal::new(false);
    let share_audio = RwSignal::new(false);
    let sharing_with_audio = RwSignal::new(false);
```

Update the `share_toggle_handler` call site to pass the two new
signals in the same order the function now expects them:

```rust
    let toggle_share = share_toggle_handler(conn.clone(), is_sharing, set_is_sharing, own_preview_hidden, share_audio.read_only(), sharing_with_audio, set_status, my_peer_id, expanded);
```

Change the existing import line:

```rust
use share::{share_supported, share_toggle_handler};
```

to:

```rust
use share::{desktop_audio_supported, share_supported, share_toggle_handler};
```

In the view, right after the existing share button (the one with
`on:click=toggle_share.clone()`, ending around the `</button>` that
closes it), add a checkbox that's hidden outside the desktop app and
disabled while already sharing:

```rust
                    <label
                        class="checkbox-field"
                        class:hidden=move || !desktop_audio_supported()
                    >
                        <input
                            type="checkbox"
                            prop:checked=share_audio
                            prop:disabled=is_sharing
                            on:change:target=move |ev| share_audio.set(ev.target().checked())
                        />
                        <span>"Compartilhar áudio"</span>
                    </label>
```

- [ ] **Step 7: Run `cargo check` for both targets again**

```bash
cargo check --features ssr
cargo check --target wasm32-unknown-unknown --features hydrate
```

Expected: both succeed with no errors.

- [ ] **Step 8: Add minimal CSS for the new checkbox**

Add to `public/styles/room.css` (next to the other `.control-group`/
`icon-btn` rules):

```css
.checkbox-field {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  color: var(--text-dim);
  font-size: 0.8rem;
  cursor: pointer;
  user-select: none;
}

.checkbox-field input[disabled] {
  cursor: not-allowed;
}
```

- [ ] **Step 9: Run it and verify manually**

```bash
cd desktop && pnpm start
```

Manually confirm, entering a room:

- The "Compartilhar áudio" checkbox is visible in the desktop app and
  disabled (grayed out, doesn't toggle) while already sharing.
- Open the same room in an ordinary browser tab (not the desktop app)
  and confirm the checkbox never appears there.
- In the desktop app, leave the checkbox unmarked and share: behavior
  identical to before this plan (no audio track), confirming nothing
  regressed.
- Mark the checkbox, play some audio on the system (e.g. a YouTube
  video in another window), and share: confirm the audio is audible
  both in a second desktop app instance/window watching, and in an
  ordinary browser tab watching the same room. The volume slider from
  the earlier per-viewer volume control sub-project should now
  actually do something audible.
- Stop sharing (via the app's own button): confirm `wpctl status` no
  longer lists the "Screen Share Audio" device.
- Repeat, but stop the share via the browser/Chromium's own native
  "Stop sharing" bar instead of the app's button: confirm the device
  is cleaned up the same way.
- Quit the whole app from the tray ("Sair") while a share with audio
  is active, without stopping the share first: confirm no
  `screen_share_audio` device lingers afterward.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml src/ui/client/webrtc.rs src/ui/pages/room/share.rs src/ui/pages/room/mod.rs public/styles/room.css
git commit -m "feat(room): opt-in system audio sharing from the desktop app"
```

---

## Definition of done

Both tasks' manual verification checklists pass. Audio sharing is
opt-in, desktop-only, cleans up its virtual device in every way a
share can end (manual stop, native browser stop, app quit), and
requires no changes on the receiving/viewer side — confirming the
per-viewer volume control sub-project already handles whatever audio
shows up. Process exclusion (choosing to leave one app out of the
shared audio) is out of scope here — see
`docs/superpowers/specs/2026-08-22-desktop-system-audio-sharing-design.md`.
