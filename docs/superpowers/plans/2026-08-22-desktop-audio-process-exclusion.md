# Desktop Audio Process Exclusion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sharing a specific window shares only that app's audio automatically; sharing the whole screen shares all system audio except whichever processes the user explicitly excludes in the picker — including processes that start making sound only after sharing has already begun.

**Architecture:** The Electron main process creates its own virtual PipeWire sink ("Screen Share Mix") and selectively `pw-link`s only the wanted processes' existing audio output ports into it, instead of tapping the whole system's default sink monitor. A window's owning process is found by resolving its X11 window ID to a PID (`xprop`); the "Compartilhar áudio" checkbox and (for whole-screen sharing) the exclusion checklist move into the picker window, decided once per share, before the picker even responds to the video request.

**Tech Stack:** Electron/TypeScript (`desktop/`), the system's `pw-loopback`, `pw-dump`, `pw-link`, `xprop` CLI tools (no new Rust crate — same approach as the previous audio-sharing plan).

## Global Constraints

- No new Rust crate.
- Sharing a specific window (picker's "Aplicativos" tab) with audio on: only that window's owning process's audio is shared, automatically, no exclusion UI shown.
- Sharing the whole screen (picker's "Tela inteira" tab) with audio on: all system audio is shared except processes checked in the exclusion list next to the checkbox.
- A process that starts playing audio *after* sharing has already started is still picked up (included if not excluded; ignored if in window-mode and not the target process) — checked on a recurring basis, not just once at share-start.
- The exclusion/inclusion choice is made once, before sharing starts — no live control while already sharing.
- Stopping a share must always clean up the virtual sink, however the share ends (button, browser's native stop control, tray quit).
- Linux only.

---

### Task 1: Electron main process — selective PipeWire audio linking engine

**Files:**
- Modify: `desktop/src/main.ts`

**Interfaces:**
- Produces (used by Task 2): `type AudioShareTarget = { mode: 'window'; pid: number } | { mode: 'screen'; excludedBinaries: string[] }`; `async function startAudioLoopback(target: AudioShareTarget): Promise<void>`; `function stopAudioLoopback(): void`; `async function resolveWindowPid(x11WindowId: number): Promise<number | null>`; `function parseX11WindowId(sourceId: string): number | null`. Also produces the IPC handlers `start-audio-loopback` (payload: `AudioShareTarget`), `stop-audio-loopback`, and `list-audio-apps` (returns `{ binary: string; label: string }[]`), all independently testable via DevTools before Task 2 wires them into the picker flow.

This task replaces the whole-system-monitor approach from the previous
audio-sharing plan. Remove the old `screen_share_audio`-based
`isLoopbackDevicePresent`/`waitForLoopbackDevice`/`start-audio-loopback`/
`stop-audio-loopback` block from `desktop/src/main.ts` entirely (the
block added `--capture-props 'media.class=Audio/Source ...'` etc.) and
replace it with what follows.

- [ ] **Step 1: Write the PipeWire introspection helpers**

Add this to `desktop/src/main.ts`, replacing the old
`audioLoopback`/`stopAudioLoopback`/`isLoopbackDevicePresent`/
`waitForLoopbackDevice` block:

```typescript
const MIX_SINK_NAME = 'screen_share_mix';
const MIX_PLAYBACK_PORTS = [
  `${MIX_SINK_NAME}:playback_FL`,
  `${MIX_SINK_NAME}:playback_FR`,
];

interface AudioStreamInfo {
  id: number;
  nodeName: string | null;
  pid: number | null;
  binary: string | null;
}

function runCollectingStdout(command: string, args: string[]): Promise<string> {
  return new Promise((resolve) => {
    const child = spawn(command, args);
    let output = '';
    child.stdout.on('data', (chunk: Buffer) => {
      output += chunk.toString();
    });
    child.on('close', () => resolve(output));
    child.on('error', () => resolve(''));
  });
}

async function listAudioOutputStreams(): Promise<AudioStreamInfo[]> {
  const output = await runCollectingStdout('pw-dump', []);
  let data: unknown;
  try {
    data = JSON.parse(output);
  } catch {
    return [];
  }
  if (!Array.isArray(data)) return [];

  const streams: AudioStreamInfo[] = [];
  for (const obj of data) {
    const props = (obj as { info?: { props?: Record<string, unknown> } })?.info?.props;
    if (!props || props['media.class'] !== 'Stream/Output/Audio') continue;
    streams.push({
      id: (obj as { id: number }).id,
      nodeName: typeof props['node.name'] === 'string' ? (props['node.name'] as string) : null,
      pid:
        props['application.process.id'] !== undefined
          ? Number(props['application.process.id'])
          : null,
      binary:
        typeof props['application.process.binary'] === 'string'
          ? (props['application.process.binary'] as string)
          : null,
    });
  }
  return streams;
}

async function isNodeNamePresent(nodeName: string): Promise<boolean> {
  const output = await runCollectingStdout('pw-dump', []);
  return output.includes(`"node.name": "${nodeName}"`);
}

async function waitForNodeName(nodeName: string, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await isNodeNamePresent(nodeName)) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out waiting for node "${nodeName}" to appear`);
}

async function listOutputPorts(nodeName: string): Promise<string[]> {
  const output = await runCollectingStdout('pw-link', ['-o']);
  const prefix = `${nodeName}:`;
  return output
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith(prefix));
}

async function linkNodeToMix(nodeName: string): Promise<void> {
  const outputs = await listOutputPorts(nodeName);
  const count = Math.min(outputs.length, MIX_PLAYBACK_PORTS.length);
  for (let i = 0; i < count; i++) {
    spawn('pw-link', [outputs[i], MIX_PLAYBACK_PORTS[i]]);
  }
}

function parseX11WindowId(sourceId: string): number | null {
  const match = sourceId.match(/^window:(\d+):/);
  return match ? parseInt(match[1], 10) : null;
}

function resolveWindowPid(x11WindowId: number): Promise<number | null> {
  return runCollectingStdout('xprop', ['-id', String(x11WindowId), '_NET_WM_PID']).then(
    (output) => {
      const match = output.match(/=\s*(\d+)/);
      return match ? parseInt(match[1], 10) : null;
    },
  );
}
```

- [ ] **Step 2: Run `pnpm exec tsc` to check for typos**

```bash
cd desktop && pnpm exec tsc
```

Expected: errors about `AudioShareTarget`/session state not existing
yet — that's Step 3. Confirm there are no *other* errors (typos in the
helpers above).

- [ ] **Step 3: Write the session lifecycle and IPC handlers**

Add this right after the helpers from Step 1:

```typescript
type AudioShareTarget =
  | { mode: 'window'; pid: number }
  | { mode: 'screen'; excludedBinaries: string[] };

interface AudioLoopbackSession {
  mixProcess: ChildProcess;
  pollInterval: NodeJS.Timeout;
  linkedNodeIds: Set<number>;
  shouldInclude: (stream: AudioStreamInfo) => boolean;
}

let audioSession: AudioLoopbackSession | null = null;

async function scanAndLink(): Promise<void> {
  if (!audioSession) return;
  const streams = await listAudioOutputStreams();
  for (const stream of streams) {
    if (audioSession.linkedNodeIds.has(stream.id)) continue;
    if (!audioSession.shouldInclude(stream)) continue;
    audioSession.linkedNodeIds.add(stream.id);
    if (stream.nodeName) {
      await linkNodeToMix(stream.nodeName);
    }
  }
}

function shouldIncludeFor(target: AudioShareTarget): (stream: AudioStreamInfo) => boolean {
  if (target.mode === 'window') {
    return (stream) => stream.pid === target.pid;
  }
  return (stream) => !stream.binary || !target.excludedBinaries.includes(stream.binary);
}

async function startAudioLoopback(target: AudioShareTarget): Promise<void> {
  if (audioSession) return;
  const mixProcess = spawn('pw-loopback', [
    '--capture-props',
    `media.class=Audio/Sink node.name=${MIX_SINK_NAME} node.description="Screen Share Mix"`,
    '--playback-props',
    'node.passive=true',
  ]);
  try {
    await waitForNodeName(MIX_SINK_NAME, 3000);
  } catch (err) {
    mixProcess.kill();
    throw err;
  }

  const session: AudioLoopbackSession = {
    mixProcess,
    linkedNodeIds: new Set(),
    shouldInclude: shouldIncludeFor(target),
    pollInterval: setInterval(() => {
      void scanAndLink();
    }, 1000),
  };
  audioSession = session;
  mixProcess.on('exit', () => {
    if (audioSession === session) {
      clearInterval(session.pollInterval);
      audioSession = null;
    }
  });

  await scanAndLink();
}

function stopAudioLoopback(): void {
  if (!audioSession) return;
  clearInterval(audioSession.pollInterval);
  audioSession.mixProcess.kill();
  audioSession = null;
}

ipcMain.handle('start-audio-loopback', (_event, target: AudioShareTarget) =>
  startAudioLoopback(target),
);

ipcMain.handle('stop-audio-loopback', () => {
  stopAudioLoopback();
});

ipcMain.handle('list-audio-apps', async () => {
  const streams = await listAudioOutputStreams();
  const seen = new Set<string>();
  const apps: { binary: string; label: string }[] = [];
  for (const stream of streams) {
    if (!stream.binary || seen.has(stream.binary)) continue;
    seen.add(stream.binary);
    apps.push({ binary: stream.binary, label: stream.binary });
  }
  return apps;
});
```

Update the existing `app.on('before-quit', ...)` block — it already
calls `stopAudioLoopback()`, which now refers to this new
implementation, so no change needed there.

- [ ] **Step 4: Compile**

```bash
pnpm exec tsc
```

Expected: no errors.

- [ ] **Step 5: Run it and verify manually**

```bash
pnpm exec electron .
```

Open the main window's DevTools (`Ctrl+Shift+I`) and, with something
audible playing on the system (e.g. a YouTube tab), run in the
console:

```js
await window.desktopAudio.start({ mode: 'screen', excludedBinaries: [] })
```

In another terminal:

```bash
wpctl status
```

Expected: a Sink named "Screen Share Mix" exists, and
`pw-link -l | grep screen_share_mix` shows the currently-playing app's
output ports linked to `screen_share_mix:playback_FL/FR`.

Now test exclusion — stop it and start again excluding that app's
binary (find the exact binary name from `pw-dump | grep
application.process.binary` while it's playing):

```js
await window.desktopAudio.stop()
await window.desktopAudio.start({ mode: 'screen', excludedBinaries: ['spotify'] })
```

(substitute the real binary name). Expected:
`pw-link -l | grep screen_share_mix` shows no link from that app.

Test dynamic pickup: with the exclusion-free session still running,
start playing audio in a *different* app that wasn't running before.
Wait ~2 seconds, then check `pw-link -l | grep screen_share_mix` again
— the new app's ports should now be linked too, without restarting
anything.

Test window mode:

```js
await window.desktopAudio.stop()
await window.desktopAudio.start({ mode: 'window', pid: 121229 })
```

(substitute a real PID of a currently-playing app, e.g. from
`pw-dump | grep application.process.id` while that app plays). Expected:
only that PID's streams get linked, confirmed the same way via
`pw-link -l`.

Finally:

```js
await window.desktopAudio.stop()
```

Expected: `wpctl status` no longer lists "Screen Share Mix", and
`pw-link -l` shows no `screen_share_mix` ports at all.

- [ ] **Step 6: Commit**

```bash
git add desktop/src/main.ts
git commit -m "feat(desktop): selectively link processes into a virtual mix sink for audio sharing"
```

---

### Task 2: Picker UI — audio checkbox and exclusion list

**Files:**
- Modify: `desktop/static/picker.html`
- Modify: `desktop/static/picker.js`
- Modify: `desktop/src/preload.ts`
- Modify: `desktop/src/main.ts`

**Interfaces:**
- Consumes: `AudioShareTarget`, `startAudioLoopback`, `parseX11WindowId`, `resolveWindowPid`, and the `list-audio-apps` IPC handler from Task 1.
- Produces: `window.picker.select({ sourceId, shareAudio, excludedBinaries })` (replaces the old `window.picker.select(id: string)`), `window.picker.listAudioApps(): Promise<{ binary: string; label: string }[]>`. `showSourcePicker()`'s resolved value changes shape — described in Step 4 below, consumed only within this same file, not exported further.

- [ ] **Step 1: Add the checkbox and exclusion panel markup to `desktop/static/picker.html`**

Add inside `<header>`, right after the `<h1>` and before `<div id="tabs">`:

```html
<label id="audio-checkbox-label">
  <input type="checkbox" id="audio-checkbox" />
  Compartilhar áudio
</label>
<div id="audio-exclude-panel" class="hidden">
  <p>Excluir do áudio:</p>
  <div id="audio-exclude-list"></div>
</div>
```

Add this CSS inside the existing `<style>` block, near the `.tab`
rules:

```css
#audio-checkbox-label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--text-dim);
  margin: 12px 0;
  cursor: pointer;
}

#audio-exclude-panel {
  background: var(--surface-2);
  border-radius: 8px;
  padding: 10px 12px;
  margin-bottom: 12px;
  font-size: 12px;
}

#audio-exclude-panel p {
  margin: 0 0 6px;
  color: var(--text-dim);
}

#audio-exclude-list label {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 0;
  cursor: pointer;
}
```

- [ ] **Step 2: Wire the checkbox and exclusion list in `desktop/static/picker.js`**

Add at the end of the file:

```javascript
const audioCheckbox = document.getElementById('audio-checkbox');
const excludePanel = document.getElementById('audio-exclude-panel');
const excludeList = document.getElementById('audio-exclude-list');
const excludedBinaries = new Set();

async function refreshExcludePanel() {
  const showPanel = audioCheckbox.checked && activeTab === 'screen';
  excludePanel.classList.toggle('hidden', !showPanel);
  if (!showPanel) return;

  const apps = await window.picker.listAudioApps();
  excludeList.innerHTML = '';
  for (const app of apps) {
    const label = document.createElement('label');
    const input = document.createElement('input');
    input.type = 'checkbox';
    input.checked = excludedBinaries.has(app.binary);
    input.addEventListener('change', () => {
      if (input.checked) {
        excludedBinaries.add(app.binary);
      } else {
        excludedBinaries.delete(app.binary);
      }
    });
    label.appendChild(input);
    label.appendChild(document.createTextNode(app.label));
    excludeList.appendChild(label);
  }
}

audioCheckbox.addEventListener('change', () => {
  void refreshExcludePanel();
});
```

Change the existing tab-switching handler so it also refreshes the
exclude panel (tabs only render the video grid today):

```javascript
for (const tab of document.querySelectorAll('.tab')) {
  tab.addEventListener('click', () => {
    activeTab = tab.dataset.tab;
    for (const t of document.querySelectorAll('.tab')) {
      t.classList.toggle('active', t === tab);
    }
    render();
    void refreshExcludePanel();
  });
}
```

Change the `select` call inside `render()`'s click handler from:

```javascript
const choose = () => window.picker.select(source.id);
```

to:

```javascript
const choose = () =>
  window.picker.select({
    sourceId: source.id,
    shareAudio: audioCheckbox.checked,
    excludedBinaries: Array.from(excludedBinaries),
  });
```

- [ ] **Step 3: Update `desktop/src/preload.ts`**

Replace the existing `window.picker` block:

```typescript
contextBridge.exposeInMainWorld('picker', {
  onSources: (callback: (sources: PickerSource[]) => void) => {
    ipcRenderer.on('picker:sources', (_event, sources: PickerSource[]) => {
      callback(sources);
    });
  },
  select: (id: string) => {
    ipcRenderer.send('picker:selected', id);
  },
});
```

with:

```typescript
interface PickerChoice {
  sourceId: string;
  shareAudio: boolean;
  excludedBinaries: string[];
}

contextBridge.exposeInMainWorld('picker', {
  onSources: (callback: (sources: PickerSource[]) => void) => {
    ipcRenderer.on('picker:sources', (_event, sources: PickerSource[]) => {
      callback(sources);
    });
  },
  select: (choice: PickerChoice) => {
    ipcRenderer.send('picker:selected', choice);
  },
  listAudioApps: () => ipcRenderer.invoke('list-audio-apps'),
});
```

Leave the existing `window.desktopAudio` block (`start`/`stop`) as-is
— still used for Task 1's manual testing and, from now on, no longer
called by the Rust side at all (Task 3 removes that call; Electron
itself starts audio from inside the picker flow, per the next step).

- [ ] **Step 4: Rewrite `showSourcePicker` and the display-media handler in `desktop/src/main.ts`**

Replace the `PickerSource` interface and `showSourcePicker` function
with:

```typescript
interface PickerSource {
  id: string;
  name: string;
  thumbnailDataUrl: string;
  iconDataUrl: string | null;
}

interface PickerChoice {
  sourceId: string;
  shareAudio: boolean;
  excludedBinaries: string[];
}

interface ShareChoice {
  source: Electron.DesktopCapturerSource;
  shareAudio: boolean;
  excludedBinaries: string[];
}

function showSourcePicker(): Promise<ShareChoice | null> {
  return new Promise((resolve) => {
    void (async () => {
      const sources = await desktopCapturer.getSources({
        types: ['screen', 'window'],
        thumbnailSize: { width: 300, height: 200 },
        fetchWindowIcons: true,
      });

      const pickerSources: PickerSource[] = sources.map((s) => ({
        id: s.id,
        name: s.name,
        thumbnailDataUrl: s.thumbnail.toDataURL(),
        iconDataUrl: s.appIcon && !s.appIcon.isEmpty() ? s.appIcon.toDataURL() : null,
      }));

      const pickerWindow = new BrowserWindow({
        width: 1000,
        height: 720,
        parent: mainWindow ?? undefined,
        frame: false,
        transparent: true,
        resizable: true,
        minWidth: 640,
        minHeight: 480,
        skipTaskbar: true,
        webPreferences: {
          preload: path.join(__dirname, 'preload.js'),
        },
      });

      let settled = false;
      const settle = (choice: PickerChoice | null) => {
        if (settled) return;
        settled = true;
        if (!choice) {
          resolve(null);
        } else {
          const source = sources.find((s) => s.id === choice.sourceId) ?? null;
          resolve(
            source
              ? {
                  source,
                  shareAudio: choice.shareAudio,
                  excludedBinaries: choice.excludedBinaries,
                }
              : null,
          );
        }
        if (!pickerWindow.isDestroyed()) pickerWindow.close();
      };

      ipcMain.once('picker:selected', (_event, choice: PickerChoice) => settle(choice));
      pickerWindow.on('closed', () => settle(null));

      // Delay arming "click outside closes it" slightly so the window
      // manager focusing this new window doesn't itself trigger a blur.
      setTimeout(() => {
        pickerWindow.on('blur', () => settle(null));
      }, 300);

      await pickerWindow.loadFile(
        path.join(__dirname, '..', 'static', 'picker.html'),
      );
      pickerWindow.webContents.send('picker:sources', pickerSources);
    })();
  });
}

async function resolveAudioTarget(chosen: ShareChoice): Promise<AudioShareTarget | null> {
  if (chosen.source.id.startsWith('window:')) {
    const x11Id = parseX11WindowId(chosen.source.id);
    if (x11Id === null) return null;
    const pid = await resolveWindowPid(x11Id);
    if (pid === null) return null;
    return { mode: 'window', pid };
  }
  return { mode: 'screen', excludedBinaries: chosen.excludedBinaries };
}
```

Replace the `setDisplayMediaRequestHandler` call inside
`app.whenReady().then(...)`:

```typescript
  session.defaultSession.setDisplayMediaRequestHandler(
    async (_request, callback) => {
      const chosen = await showSourcePicker();
      callback(chosen ? { video: chosen } : {});
    },
  );
```

with:

```typescript
  session.defaultSession.setDisplayMediaRequestHandler(
    async (_request, callback) => {
      const chosen = await showSourcePicker();
      if (!chosen) {
        callback({});
        return;
      }
      if (chosen.shareAudio) {
        const target = await resolveAudioTarget(chosen);
        if (target) {
          try {
            await startAudioLoopback(target);
          } catch {
            // Proceed with video-only rather than failing the whole share.
          }
        }
      }
      callback({ video: chosen.source });
    },
  );
```

- [ ] **Step 5: Compile**

```bash
cd desktop && pnpm exec tsc
```

Expected: no errors.

- [ ] **Step 6: Run it and verify manually**

```bash
pnpm exec electron .
```

With something audible playing:

- Open the picker, go to "Aplicativos", check "Compartilhar áudio" —
  confirm no exclusion panel appears.
- Pick the window belonging to whatever's making sound (e.g. Spotify's
  window). Check `wpctl status` and `pw-link -l | grep
  screen_share_mix` — only that app's process should be linked.
- Open the picker again, go to "Tela inteira", check "Compartilhar
  áudio" — confirm the exclusion panel appears listing currently
  playing apps.
- Check one app to exclude, pick "Tela inteira" as the source. Confirm
  via `pw-link -l` that every *other* currently-playing app got linked
  and the excluded one didn't.
- Leave the checkbox unchecked entirely and share either tab — confirm
  no "Screen Share Mix" sink gets created at all (`wpctl status` shows
  nothing new).

- [ ] **Step 7: Commit**

```bash
git add desktop/static/picker.html desktop/static/picker.js desktop/src/preload.ts desktop/src/main.ts
git commit -m "feat(desktop): move audio sharing choice into the source picker"
```

---

### Task 3: Simplify the Rust side

**Files:**
- Modify: `src/ui/client/webrtc.rs`
- Modify: `src/ui/pages/room/share.rs`
- Modify: `src/ui/pages/room/mod.rs`

**Interfaces:**
- Produces: `capture_display() -> Result<MediaStream, JsValue>` (no arguments — replaces the `share_audio: bool` version). `share_toggle_handler`/`stop_sharing` go back to their pre-audio-sharing signatures (no `share_audio`/`sharing_with_audio` parameters).

The audio decision is now made entirely inside the Electron picker
(Task 2), before `getDisplayMedia()` even resolves. The Rust side no
longer decides *whether* to try for audio — it always tries, and
treats "no such device" as the normal "audio wasn't requested" case
rather than an error.

- [ ] **Step 1: Simplify `capture_display` in `src/ui/client/webrtc.rs`**

Replace the `capture_display` function:

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
```

with:

```rust
pub async fn capture_display() -> Result<MediaStream, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window: not running in a browser"))?;
    let media_devices = window.navigator().media_devices()?;

    let constraints = DisplayMediaStreamConstraints::new();
    constraints.set_video_bool(true);

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(promise).await?;
    let video_stream = stream.dyn_into::<MediaStream>()?;

    // Whether to also attach audio was already decided inside the
    // desktop app's own share picker, before this call was even made —
    // this just tries, and a missing device means audio wasn't
    // requested (not an error) rather than something to report.
    match capture_loopback_audio(&media_devices).await {
        Ok(audio_stream) => combine_video_and_audio(&video_stream, &audio_stream),
        Err(_) => Ok(video_stream),
    }
}
```

Delete the `start_desktop_audio_loopback` function entirely (no
longer called from Rust — Electron starts audio itself inside the
picker flow now).

Change the device-label match inside `capture_loopback_audio` from:

```rust
        if info.kind() == web_sys::MediaDeviceKind::Audioinput
            && info.label().contains("Screen Share Audio")
        {
```

to:

```rust
        if info.kind() == web_sys::MediaDeviceKind::Audioinput
            && info.label().contains("Screen Share Mix")
        {
```

(The device is now a sink's auto-generated monitor rather than a
dedicated source node, so its exact label text may come out as
something like "Monitor of Screen Share Mix" rather than "Screen Share
Mix" verbatim — `.contains()` matches either way. Step 4 below confirms
the real label and adjusts this string if it doesn't actually contain
"Screen Share Mix".)

Keep `stop_desktop_audio_loopback` exactly as it is today (still
called from `share.rs`).

- [ ] **Step 2: Update `src/ui/pages/room/share.rs`**

Remove the `desktop_audio_supported` function entirely (both
`cfg` variants).

Change `share_toggle_handler`'s signature (both variants) — remove
the `share_audio: ReadSignal<bool>` and `sharing_with_audio:
RwSignal<bool>` parameters:

```rust
#[cfg(not(feature = "hydrate"))]
pub(super) fn share_toggle_handler(
    _conn: RoomConnection,
    _is_sharing: ReadSignal<bool>,
    _set_is_sharing: WriteSignal<bool>,
    _own_preview_hidden: RwSignal<bool>,
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
    set_status: WriteSignal<String>,
    my_peer_id: ReadSignal<Option<String>>,
    expanded: RwSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
```

Inside the `hydrate` version, change:

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

to:

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

A few lines down, remove this line entirely (it no longer applies —
there's no local "did this session have audio" flag to set):

```rust
            sharing_with_audio.set(share_audio_value);
```

Inside the `onended` closure, change:

```rust
                let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                    stop_sharing(&conn_for_end, set_is_sharing, own_preview_hidden, sharing_with_audio, expanded, my_peer_id);
                });
```

to:

```rust
                let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                    stop_sharing(&conn_for_end, set_is_sharing, own_preview_hidden, expanded, my_peer_id);
                });
```

Change `stop_sharing`'s signature — remove `sharing_with_audio:
RwSignal<bool>` and call `stop_desktop_audio_loopback`
unconditionally instead of only when a flag was set:

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

to:

```rust
#[cfg(feature = "hydrate")]
pub(super) fn stop_sharing(
    conn: &RoomConnection,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    expanded: RwSignal<Option<String>>,
    my_peer_id: ReadSignal<Option<String>>,
) {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;

    // Always attempt this — it's a no-op in Electron if no audio
    // session was ever started, and this path also runs in a plain
    // browser (no `window.desktopAudio` there), where it's likewise a
    // harmless no-op inside `stop_desktop_audio_loopback` itself.
    spawn_local(async {
        let _ = crate::ui::client::webrtc::stop_desktop_audio_loopback().await;
    });

    if let Some(stream) = conn.local_stream.borrow_mut().take() {
```

- [ ] **Step 3: Update `src/ui/pages/room/mod.rs`**

Remove the `share_audio` and `sharing_with_audio` signal
declarations:

```rust
    let share_audio = RwSignal::new(false);
    let sharing_with_audio = RwSignal::new(false);
```

Change the `share_toggle_handler` call site back down to the shorter
argument list:

```rust
    let toggle_share = share_toggle_handler(conn.clone(), is_sharing, set_is_sharing, own_preview_hidden, share_audio.read_only(), sharing_with_audio, set_status, my_peer_id, expanded);
```

becomes:

```rust
    let toggle_share = share_toggle_handler(conn.clone(), is_sharing, set_is_sharing, own_preview_hidden, set_status, my_peer_id, expanded);
```

Change the import line:

```rust
use share::{desktop_audio_supported, share_supported, share_toggle_handler};
```

to:

```rust
use share::{share_supported, share_toggle_handler};
```

Remove the checkbox markup added by the previous plan — delete this
whole block from the view:

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

- [ ] **Step 4: Run `cargo check` for both targets**

```bash
cargo check --features ssr
cargo check --target wasm32-unknown-unknown --features hydrate
```

Expected: both succeed. If either fails on something in
`dev_preview.rs`, that file never called `share_toggle_handler`
directly (only `member_cards`), so it shouldn't need changes — but
check its imports/signal list if an error points there.

- [ ] **Step 5: Run it end-to-end and verify manually**

```bash
cd desktop && pnpm exec tsc && pnpm exec electron .
```

- Confirm the room's control bar no longer shows a "Compartilhar
  áudio" checkbox (it's only in the picker now, from Task 2).
- Share a window with audio checked in the picker: confirm real sound
  from that app reaches a second viewer (desktop or plain browser).
  While this is running, check the browser DevTools console for any
  errors, and if audio doesn't come through, check
  `navigator.mediaDevices.enumerateDevices()`'s actual labels — if
  none contains "Screen Share Mix", adjust the match string in
  `capture_loopback_audio` (Step 1 above) to whatever substring is
  actually common to the real label and re-run this step.
- Share the whole screen with audio checked and one app excluded:
  confirm that app's sound is absent for viewers while everything else
  comes through.
- Share without checking the audio box at all: confirm behavior is
  identical to before any of the three audio sub-projects — no audio
  track, no errors.
- Stop sharing (button, then separately test the browser's native
  "Stop sharing" control): confirm `wpctl status` shows no lingering
  "Screen Share Mix" sink either way.

- [ ] **Step 6: Commit**

```bash
git add src/ui/client/webrtc.rs src/ui/pages/room/share.rs src/ui/pages/room/mod.rs
git commit -m "feat(room): resolve audio sharing choice entirely from the desktop picker"
```

---

## Definition of done

All three tasks' manual verification checklists pass. Window-share
audio is automatic and single-process; screen-share audio respects an
exclusion list chosen once at share-start; a process that starts
playing sound mid-share is still picked up correctly for both modes;
stopping a share (any way it can end) always cleans up the virtual
sink. This completes the three-part audio feature set alongside
per-viewer volume control and opt-in system audio sharing.
