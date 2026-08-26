# Windows Audio Sharing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Tasks 2, 3, and 6 require an actual Windows 10 (2004+) or Windows 11 machine to verify — they cannot be completed or tested from Linux.

**Goal:** Bring the Linux desktop app's audio-sharing feature set to Windows
with full parity: sharing a specific window shares only that window's
process's audio automatically; sharing the whole screen shares all system
audio except whichever processes are excluded in the picker; a process that
starts making sound only after sharing has begun is still picked up; the
app's own audio (e.g. watching a share on the same machine) never leaks back
into the mix. No behavior change on Linux, and no changes at all to the
picker UI (`picker.html`/`picker.js`) or the `AudioShareTarget` IPC
contract — this plan is purely a second backend behind the same interface.

**Why this shape, and what was ruled out:** see "Design rationale" below for
the alternatives considered (a third-party Node addon, pure-JS FFI) and why
a focused Rust native module is the only one that's actually stable and
maintainable here — the short version is that a raw FFI approach means
hand-implementing a COM callback interface with no memory safety, and the
one existing pre-built Node addon that does something similar
(`electron-native-screenshare`) still leaves the hardest part (getting
captured audio into a `MediaStreamTrack`) entirely unsolved, on top of being
unproven (0 stars, capture path untested in its own CI) and requiring a
native build toolchain on every install.

**Architecture:**

- A small Rust crate (`desktop/native/windows-audio/`), built with
  `napi-rs` on top of the `wasapi` crate
  (https://docs.rs/wasapi/latest/wasapi/), owns every Windows-specific piece:
  resolving a window handle to its owning process, enumerating which
  processes currently hold an active WASAPI audio session, and capturing +
  mixing PCM from each qualifying one via
  `AudioClient::new_application_loopback_client`.
- Both window mode and screen mode are implemented as **the same
  mechanism** — continuously enumerate active audio sessions, capture (in
  WASAPI's `INCLUDE` mode) every process whose resolved executable name
  passes a predicate, and mix whatever's currently active together. Window
  mode's predicate is "name equals the target's name"; screen mode's is
  "name is not in the excluded list." This mirrors the Linux
  implementation's own model (`shouldIncludeFor` in
  `audio/loopback-session.ts`) closely on purpose — same self-exclusion
  rule, same "re-scan on a poll so new processes get picked up" behavior —
  and deliberately avoids WASAPI's own `EXCLUDE_TARGET_PROCESS_TREE` mode,
  which only ever targets one process (tree) at a time and can't express
  "exclude these three unrelated apps." It also avoids relying on
  `include_tree` process-tree walking to find a window's real
  audio-producing process — Chromium-based apps (any browser, and this
  app) route audio through a subprocess that isn't reliably a descendant of
  the window's own process on every platform (confirmed empirically on
  Linux with Brave; unverified but plausible on Windows too — see
  "Verification risk" under Task 2). Matching by resolved name instead of
  by PID/tree sidesteps that uncertainty entirely, the same way binary-name
  matching already fixed this exact class of bug on Linux.
- The native module never touches the browser or Electron's renderer
  directly. It hands mixed PCM chunks to the Electron **main** process via
  a `napi` threadsafe callback; the main process forwards each chunk to the
  renderer over the existing IPC bridge (as a transferable `ArrayBuffer` —
  explicitly a supported, cloneable type across `contextBridge`, unlike a
  `MediaStreamTrack` or any other DOM object, which are not).
- The **renderer** (the actual Leptos/WASM app, in `webrtc.rs`) turns that
  incoming PCM stream into a real `MediaStreamTrack` using
  `MediaStreamTrackGenerator` + `AudioData` — a Chromium API built
  specifically for "I have raw audio from a non-browser source, make me a
  track out of it." This was chosen over the alternative
  (`AudioWorkletNode` + `MediaStreamAudioDestinationNode`) because it needs
  no separate worklet-processor module, no `Blob`/`ObjectURL` bookkeeping,
  and no hand-rolled ring buffer inside a real-time audio callback — it's
  meaningfully less code for the same result. The resulting track is
  combined with the video track through the **exact same**
  `combine_video_and_audio()` already used on Linux — no changes needed
  there at all.
- Everything above sits entirely behind the same `AudioShareTarget` type
  and the same three IPC channel names (`start-audio-loopback`,
  `stop-audio-loopback`, `list-audio-apps`) the Linux implementation
  already established. `main.ts`'s composition root picks the Linux or
  Windows backend once, based on `process.platform`; nothing else needs to
  know which platform it's running on.

**Tech Stack:** Rust + `napi-rs` + the `wasapi` crate for the native
module (Windows-only; nothing here touches the Linux build). TypeScript,
extending the existing modular `desktop/src/audio/` structure. Rust/
wasm-bindgen (`web-sys` + `js_sys` reflection, matching this codebase's
existing pattern for reaching into browser APIs `web-sys` doesn't fully
cover) for the track-construction piece.

## Design rationale — alternatives considered and ruled out

**Pure-JS FFI (`koffi`), no compiled Rust/C++ at all.** Genuinely the right
tool for flat Win32 calls — `GetWindowThreadProcessId` has no business being
implemented in Rust when a one-line FFI call does the same thing with a
prebuilt binary and zero compilation step for anyone installing the app.
This plan uses exactly that reasoning for the *simple* pieces, but WASAPI's
process-loopback activation is COM-based: it requires implementing
`IActivateAudioInterfaceCompletionHandler` (a real COM interface, meaning a
hand-built vtable of function pointers Windows calls back into) and walking
`IAudioClient`/`IAudioCaptureClient` vtables by hand. `koffi` can technically
do this (it supports JS-function-as-native-callback), but there is no type
checking, no safety net, and a wrong struct offset doesn't error — it
corrupts memory. The `wasapi` crate already does this exact dance safely,
is maintained, and is used by other real projects. Re-deriving it by hand
in raw FFI has no upside and real downside.

**`electron-native-screenshare`** (the third-party npm package
investigated earlier). Ruled out for three reasons: (1) it only delivers
raw PCM via a callback — it does not solve the "turn this into a
`MediaStreamTrack`" problem either, so adopting it wouldn't remove any of
this plan's real work, just add a dependency; (2) it requires a
`node-gyp`/C++ build toolchain on every install, which the current Linux
implementation has zero of; (3) it's unproven (0 stars, ~2 months old,
single author, its own audio-capture tests are skipped in CI for lack of a
virtual device) and its Linux exclude-mode appears to share the exact
split-node/no-PID bug this project already found and fixed by hand on
PipeWire — a bug we'd be trusting, not verifying.

**A pre-built virtual audio device (a VB-Cable-style approach).** Would
avoid the PCM-bridging problem entirely by making captured audio show up as
a normal, `getUserMedia`-selectable device, mirroring exactly what the
Linux implementation does with a PipeWire sink. Not viable: Windows has no
userland way to register a new system-visible audio endpoint the way
PipeWire lets any process do — that requires a signed kernel-mode audio
driver, which is out of scope by a wide margin (WHQL signing, driver
maintenance, admin rights to install).

## Global Constraints

- Windows 10 version 2004+ (the minimum OS version for WASAPI process-loopback
  activation) or Windows 11.
- No new virtual audio device, no kernel-mode component, no code-signing
  pipeline beyond what packaging the app already needs.
- No changes to `desktop/static/picker.html` / `picker.js`, the
  `AudioShareTarget` type in `shared-types.ts`, or the three existing IPC
  channel names — the Windows backend is a drop-in behind the same
  contract the picker and `display-media-handler.ts` already speak.
- No changes to the Linux code path at all; every new module is additive
  and selected only when `process.platform === 'win32'`.
- Task 1 is deliberately platform-independent and should be done first —
  it's the one piece of this plan with no real precedent anywhere in this
  codebase, and it can be fully proven out on Linux (or any machine running
  the desktop app) before any Windows-specific code exists.

---

### Task 1: Prove `MediaStreamTrackGenerator` + `AudioData` actually works

**Why first:** every other task assumes externally-sourced raw PCM can
become a real, WebRTC-attachable `MediaStreamTrack` this way. Nothing else
in this codebase has ever done that. It's fully decoupled from WASAPI/
Windows — it only needs a Chromium renderer, so it can be verified today,
on this machine, with synthetic data, before committing to the rest of the
architecture around it.

> **Verified 2026-08-26** (Chrome 150, via `cargo leptos watch` +
> browser automation, DevTools console) — see Step 5 for the full
> writeup, including one unresolved ambiguity in the Step 4 result.

- [x] **Step 1: Confirm web-sys has the needed bindings**

  Already confirmed for this plan (web-sys 0.3.104): `AudioContext`,
  `MediaStreamTrackGenerator`, `MediaStreamTrackGeneratorInit`,
  `AudioData`, `AudioDataInit` all exist as web-sys structs, each behind a
  same-named Cargo feature. `WritableStream` /
  `WritableStreamDefaultWriter` do **not** have web-sys bindings — getting
  `.writable.getWriter()` and calling `.write(...)` needs `js_sys::Reflect`
  dynamic calls, the same pattern `is_desktop_app()`/
  `capture_loopback_audio()` already use in `webrtc.rs` for reaching into
  `window.desktopAudio`.

- [x] **Step 2: Synthetic tone via DevTools, entirely in-browser**

  Open the running app (browser or the desktop shell) and its DevTools
  console. Run:

  ```js
  const generator = new MediaStreamTrackGenerator({ kind: 'audio' });
  const writer = generator.writable.getWriter();
  const sampleRate = 48000;
  let frameCount = 0;
  const interval = setInterval(async () => {
    const frames = 960; // 20ms @ 48kHz
    const data = new Float32Array(frames * 2); // stereo
    for (let i = 0; i < frames; i++) {
      const t = (frameCount + i) / sampleRate;
      const sample = Math.sin(2 * Math.PI * 440 * t) * 0.2;
      data[i * 2] = sample;
      data[i * 2 + 1] = sample;
    }
    frameCount += frames;
    const audioData = new AudioData({
      format: 'f32',
      sampleRate,
      numberOfFrames: frames,
      numberOfChannels: 2,
      timestamp: (frameCount / sampleRate) * 1_000_000, // microseconds
      data,
    });
    await writer.write(audioData);
  }, 20);
  ```

  Expected: no thrown errors, `writer.write()`'s promise resolves each
  time without ballooning latency (a sign of correct backpressure
  handling).

- [x] **Step 3: Confirm it's audible and glitch-free**

  ```js
  const audioCtx = new AudioContext();
  const src = audioCtx.createMediaStreamSource(new MediaStream([generator]));
  src.connect(audioCtx.destination);
  ```

  Expected: a clean 440Hz tone, no clicking/stuttering. If there's
  stuttering, try a larger frame size (e.g. 1920 frames / 40ms) before
  concluding the approach doesn't work — this is the kind of tuning knob
  expected to need adjusting, not a sign of a fundamentally wrong
  approach.

- [x] **Step 4: Confirm it survives a real WebRTC round-trip** (partial — see below)

  In an actual room (two tabs/peers, same as this project's existing
  manual test flow), attach `generator.track` alongside a real video track
  to the local stream a sharer sends, and confirm a second peer watching
  hears the tone. This is the real proof — a track that merely plays
  locally but fails once handed to an `RTCPeerConnection` would sink this
  whole approach.

  Verified with two `RTCPeerConnection`s (loopback, same tab, real
  offer/answer/ICE, `connectionState: "connected"`) rather than two actual
  room tabs — equivalent for this specific question and faster to
  automate. `ontrack` fired, `pc1`'s `media-source` stats showed
  `audioLevel≈0.2` and real `totalAudioEnergy` (matching the synthetic
  tone), and RTP packets were actually sent (`bytesSent`/`packetsSent`
  climbing steadily). **But** `pc2`'s `inbound-rtp` stats showed
  `audioLevel: 0`, `totalAudioEnergy: 0`, `totalSamplesReceived: 0` the
  entire time, despite `bytesReceived`/`packetsReceived` climbing too —
  i.e. packets demonstrably arrive, but the decoded/rendered signal reads
  as silence. Root cause not conclusively identified: the most likely
  explanation is that this sandbox's Chrome has no real audio output
  device (`enumerateDevices()` lists an audioinput/audiooutput pair but
  both have empty labels — consistent with a null/fake audio backend
  under CDP automation, not real hardware), and Chrome's inbound-rtp
  audio-energy stats are computed off the real output device's render
  callback, which never fires without one. This is a known class of
  false-negative in headless/automated Chrome and was not distinguishable
  from a genuine bug without real hardware. **This must be re-checked
  early on the Windows VM** (or any machine with real speakers) before
  trusting audio actually reaches a real second peer — everything
  upstream of the network hop (API surface, write() backpressure, local
  playback amplitude, sender-side encoding) checked out cleanly, so if the
  Windows recheck also shows silence, suspect the receive path
  specifically rather than re-litigating Steps 1-3.

- [x] **Step 5: Write down what was actually true in practice**

  Record: the exact `format` string that worked (`'f32'` vs `'f32-planar'`
  — this project's mixed/interleaved PCM from Task 3 needs the matching
  one), whether `timestamp` needed to be strictly monotonic non-negative,
  and the largest safe delay between chunks before the browser starts
  glitching. Task 5 depends on these being right the first time, not
  rediscovered later.

  - `format: 'f32'` (interleaved stereo) worked as constructed — no
    `AudioData` constructor errors, matches Task 3's planned mixer output
    (one interleaved f32 stereo buffer per chunk), so Task 5 should use
    `'f32'`, not `'f32-planar'`.
  - `timestamp` was fed as a monotonically increasing microsecond counter
    derived from cumulative frames written (`frameCount / sampleRate *
    1_000_000`), starting just above 0 (first chunk's timestamp is one
    chunk-duration in, not exactly 0). No errors or scheduling issues
    observed with this scheme over ~500 consecutive writes.
  - Chunk cadence: 20ms/960 frames-per-chunk (this plan's target cadence,
    matching Task 3's mixer) produced no backpressure at all —
    `writer.write()` resolved in ~0.01-0.1ms consistently, nowhere near
    ballooning. Never had to test a larger frame size; 20ms is fine.
  - No `writer.write()` promise ever rejected across ~700 chunks in this
    session (0 errors recorded).

**If this fails:** fall back to `AudioWorkletNode` +
`createMediaStreamDestination()` — more moving parts (a separate processor
module loaded via `audioWorklet.addModule()`, needs a `Blob`+`ObjectURL`
built at runtime since this project has no bundler step for arbitrary JS
files, and a hand-rolled ring buffer inside the worklet's real-time
`process()` callback) but uses only APIs already known to work broadly.
Don't build this speculatively — only fall back to it if Task 1 genuinely
fails.

---

### Task 2: Scaffold the native Windows audio module — process/window resolution

> **Implemented 2026-08-26**, Steps 1-4. Step 5 (manual verification)
> still needs the real Windows VM — see the note there.

**Files:**
- New: `desktop/native/windows-audio/Cargo.toml`
- New: `desktop/native/windows-audio/src/lib.rs`
- New: `desktop/native/windows-audio/src/process_identity.rs` (Task 2)
- New: `desktop/native/windows-audio/src/capture.rs` (Task 3)

**Interfaces:**
- Produces: `#[napi] fn list_active_audio_processes() -> Result<Vec<AudioProcessInfo>>` where `AudioProcessInfo { pid: u32, exe_name: String }`; `#[napi] fn get_pid_for_window(hwnd: i64) -> Result<Option<u32>>`.

**Verification risk to resolve here, not later:** on Linux, a browser
window's owning PID (via `_NET_WM_PID`) and its actual audio-producing PID
(the browser's Audio Service subprocess) turned out to be different
processes, but a parent/child pair — confirmed live with Brave (window PID
7060, audio PID 7675, `ppid(7675) == 7060`). Whether Chromium's
process-tree topology on **Windows** puts the Audio Service subprocess as a
descendant of the *window's own* process (as opposed to a sibling under
the main browser process, with the window itself belonging to a sandboxed
renderer) is not yet known and can only be checked on real Windows
hardware. This is exactly why window mode's design (Task 3) matches by
**resolved executable name**, not by PID or process tree — so this
ambiguity doesn't matter either way. Confirm during this task's manual
verification, but the design doesn't depend on the answer.

- [x] **Step 1: Scaffold via `napi-rs`'s own generator** (deviation — see below)

  ```bash
  cd desktop/native
  npx @napi-rs/cli new windows-audio
  ```

  This generates the `package.json`/`build.rs`/CI-template scaffolding
  that produces prebuilt `.node` binaries per target and auto-generates
  the TypeScript type definitions from the `#[napi]` annotations — this is
  most of the boilerplate reduction this plan is optimizing for; avoid
  hand-writing any of it.

  **Deviation:** `napi new` (v3.8.6) always drops into an interactive
  `@inquirer/core` prompt for the min-node-api choice regardless of
  `--min-node-api`/`--dry-run`/piped stdin — tried plain pipes, `script
  -qc` for a real pty, and typed-ahead answers; the prompt library closes
  with `ExitPromptError` the moment stdin isn't a real interactive
  terminal, which this sandboxed shell can't provide. Hand-wrote the
  scaffold instead, following current napi-rs v3 conventions (verified
  against the real, installed `napi`/`napi-derive`/`napi-build` v3 source
  in the Cargo registry cache rather than from memory): `Cargo.toml`,
  `build.rs` (calling `napi_build::setup()`), `package.json` with a
  `napi.targets: ["x86_64-pc-windows-msvc"]` block, and an
  `npm/win32-x64-msvc/package.json` platform stub — the same shape `napi
  new`/`napi build --platform` produce. `index.js`/`index.d.ts` are left
  to be generated by `napi build` itself (gitignored) rather than
  hand-written, since those really are pure codegen output.

  This was then validated for real, not just eyeballed: installed the
  `x86_64-pc-windows-gnu` Rust target (available on Linux, unlike `-msvc`)
  and ran `cargo check --target x86_64-pc-windows-gnu` (with a dummy
  `libnode.dll` on `LIBNODE_PATH` to satisfy `napi-build`'s gnu-only
  link-search step, which `cargo check` never actually needs to resolve
  since it doesn't link) — this type-checks every line against the real
  `windows`/`wasapi`/`napi` crate APIs. It compiles clean, and
  `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D
  warnings` and `cargo fmt --check` both pass. This is a good proxy for
  the real `-msvc` target (`windows`/`wasapi` expose the same API on both
  Windows environments; `napi-build`'s gnu-specific libnode-linking branch
  is the only environment-specific code path involved, and it's confirmed
  dead on `-msvc`) but it is **not** a substitute for actually linking and
  running on Windows — do that first, before trusting this further.

- [x] **Step 2: Add dependencies**

  `Cargo.toml`: `napi`, `napi-derive`, `wasapi`, and the `windows` crate
  (for `GetWindowThreadProcessId`, `QueryFullProcessImageNameW` — both
  flat Win32 calls, no COM needed for either).

  Used the latest crates.io versions as of 2026-08-26: `napi` 3.12.2,
  `napi-derive` 3.6.3, `napi-build` 2.4.1, `wasapi` 0.24.0, `windows`
  0.62.2.

- [x] **Step 3: Implement `get_pid_for_window`** (signature updated — see below)

  ```rust
  use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
  use windows::Win32::Foundation::HWND;

  #[napi]
  pub fn get_pid_for_window(hwnd: i64) -> Option<u32> {
      let mut pid: u32 = 0;
      unsafe {
          GetWindowThreadProcessId(HWND(hwnd as isize), Some(&mut pid));
      }
      (pid != 0).then_some(pid)
  }
  ```

  **Deviation:** in `windows` 0.62.2 (this plan's originally-cited version
  range didn't pin one; the current release was used — see Step 2),
  `HWND` wraps `*mut c_void`, not `isize` — `HWND(hwnd as isize)` doesn't
  compile. Implemented in `src/process_identity.rs` as
  `HWND(hwnd as isize as *mut c_void)` instead, confirmed against the
  crate's real struct definition, not just against this snippet.

- [x] **Step 4: Implement `list_active_audio_processes`**

  Following the `wasapi` crate's own `examples/processes.rs`: enumerate
  `Direction::Render` devices via `DeviceEnumerator`, for each device get
  `get_iaudiosessionmanager()` → `get_audiosessionenumerator()`, iterate
  sessions, skip any where `get_state() != SessionState::Active`, resolve
  `get_process_id()` for the rest. Resolve each PID's executable filename
  via `QueryFullProcessImageNameW` (open the process with
  `PROCESS_QUERY_LIMITED_INFORMATION`, not the more privileged
  `PROCESS_QUERY_INFORMATION`, to avoid needing elevated rights for other
  users' processes) and take the file name off the end of the path.
  Dedupe by resolved name — this becomes the shape `list-audio-apps`
  already returns (`{ binary, label }[]`), just resolved differently than
  on Linux.

  Implemented in `src/process_identity.rs`. Every line checked against
  the real `wasapi`/`windows` 0.62 API surface in the Cargo registry
  cache — the `examples/processes.rs` and `examples/record_application.rs`
  files bundled with the `wasapi` crate itself matched this plan's
  description closely and were used directly as the reference.

- [ ] **Step 5: Manual verification (needs a real Windows machine)**

  ```
  cd desktop/native/windows-audio
  npm run build
  node -e "console.log(require('./index.js').listActiveAudioProcesses())"
  ```

  With Spotify (or any app) playing, confirm it shows up with the right
  exe name. Play something in a Chromium browser, note whether its
  resolved PID's exe name matches the *browser's* main exe or something
  else — this confirms/denies the "which process actually owns the
  session" question above empirically, the same way it was confirmed on
  Linux via `ps`/`pw-dump` earlier in this project.

  Then, with a window of that same browser open, get its HWND (Chromium's
  own `desktopCapturer` source `id` is `window:<hwnd>:0` — parse it with
  the same regex Linux already uses) and confirm `get_pid_for_window`
  returns a plausible PID for it.

---

### Task 3: The capture-and-mix engine

> **Implemented 2026-08-26**, Steps 1-4, in `src/capture.rs`. Step 5
> (manual verification) still needs the real Windows VM.

**Files:**
- Modify: `desktop/native/windows-audio/src/lib.rs` (or split into a
  `capture.rs` module within the same crate) — split into `capture.rs`,
  as the plan's own parenthetical allows.

**Interfaces:**
- Produces: `#[napi] struct WindowsAudioSession` with `start(mode: String, target_name: Option<String>, excluded_names: Vec<String>, on_chunk: ThreadsafeFunction<Buffer>) -> Result<()>` and `stop(&mut self)`. `mode` is `"window"` or `"screen"`, mirroring `AudioShareTarget`'s two variants.

- [x] **Step 1: The predicate**

  ```rust
  fn should_include(mode: &str, target_name: &Option<String>, excluded_names: &[String], own_exe_name: &str, candidate_name: &str) -> bool {
      if candidate_name == own_exe_name { return false; } // self-exclusion, always
      match mode {
          "window" => target_name.as_deref() == Some(candidate_name),
          _ => !excluded_names.iter().any(|n| n == candidate_name),
      }
  }
  ```

  `own_exe_name` is resolved once via `std::env::current_exe()` at module
  load, the same role `OWN_BINARY_NAME` plays on Linux.

  Implemented essentially verbatim (`capture::should_include`), plus a
  small unit-testable `own_exe_name()` helper. Not unit-tested from this
  sandbox (no `cargo test` target for `windows-gnu` without a linker) —
  worth adding a `#[cfg(test)]` table test for the four
  mode/target/excluded/self combinations once this can run somewhere with
  a full toolchain (even non-Windows would do, since this function has no
  Windows-specific logic at all).

- [x] **Step 2: The poll loop**

  A background thread (`initialize_mta()` first — process-loopback
  activation needs MTA), looping every ~1s (matching the Linux poll
  cadence in `scanAndLink`): re-run `list_active_audio_processes()`, for
  every PID whose name passes `should_include` and doesn't already have a
  running capture, spawn one (`AudioClient::new_application_loopback_client(pid, true)`,
  `StreamMode::EventsShared` per the crate's own example, requesting f32/
  48kHz/stereo explicitly so every concurrent capture shares one format
  and can be summed directly without per-stream resampling). Track running
  captures in a `HashMap<u32, CaptureHandle>`; when a previously-included
  PID's session goes away or stops being active, tear its capture down.

  Implemented as `run_poll_loop` + `CaptureHandle` (one OS thread per
  active capture, each pulling frames via its own WASAPI event handle —
  necessary because `wasapi::Handle::wait_for_event` blocks on one
  specific handle, so N concurrent captures need N threads rather than
  one poll loop juggling all of them). `CaptureHandle::drop` signals and
  joins its thread, so removing an entry from the `HashMap` is itself the
  teardown.

- [x] **Step 3: The mixer**

  A fixed-cadence loop (every ~20ms, matching Task 1's chunk size)
  drains whatever's currently buffered from every active per-PID capture,
  sums samples position-by-position with a soft clip (e.g.
  `(a + b).clamp(-1.0, 1.0)` is fine to start; a proper limiter can come
  later if clipping turns out audible in practice), and invokes
  `on_chunk` with one interleaved `f32` stereo buffer. Streams that
  briefly have nothing buffered contribute silence for that tick rather
  than stalling the whole mix — matches this project's existing
  "best-effort, not sample-perfect" standard for the Linux mix, which
  also doesn't attempt drift correction.

  Implemented as `run_mixer`, on its own thread, reading each capture's
  buffer through a `SharedBuffers` map (`Arc<Mutex<HashMap<u32,
  Arc<Mutex<VecDeque<f32>>>>>>`) that the poll loop keeps in sync with its
  own `CaptureHandle` map — the mixer thread never touches capture
  lifecycle (spawn/stop), only reads buffers, so the two loops don't need
  to coordinate on anything beyond that one shared map.

- [x] **Step 4: `stop()`**

  Signal the poll thread to exit, join it, tear down every still-running
  per-PID capture and the mixer.

  `SessionHandle::stop` flips one shared `AtomicBool` and joins both the
  poll thread and the mixer thread; the poll thread's own exit drops its
  `HashMap<u32, CaptureHandle>`, which is what actually stops and joins
  every per-PID capture thread (via `CaptureHandle::drop`).

- [ ] **Step 5: Manual verification (Windows machine required)**

  Mirrors how the Linux mix was verified earlier in this project: write
  a tiny throwaway Node script that calls `start()` with a no-op
  `on_chunk` that appends every chunk to a `.raw` file, run it while
  multiple apps play distinguishable audio (e.g. two different YouTube
  videos), stop after a few seconds, and play the result back with
  `ffplay -f f32le -ar 48000 -ac 2 out.raw` — confirm both sources are
  audible together, correctly synced, without clipping distortion.
  Then re-run with one of the two apps' name in `excluded_names` and
  confirm only the other one is present. Then start a *third* app's
  audio only *after* capture has already started, and confirm it gets
  picked up within about a second (the mid-share pickup requirement).

---

### Task 4: Wire the native module into the Electron TypeScript layer

> **Implemented 2026-08-26.** `pnpm exec tsc` is clean. See per-step notes
> for real deviations from the plan's illustrative snippets.

**Files:**
- New: `desktop/src/audio/windows/process-identity.ts`
- New: `desktop/src/audio/windows/loopback-session.ts`
- Modify: `desktop/src/audio/ipc-handlers.ts`
- Modify: `desktop/src/display-media-handler.ts`
- Modify: `desktop/src/preload.ts`

**Interfaces:**
- Consumes: the native module from Task 2/3.
- Produces: `startAudioLoopback`/`stopAudioLoopback`/`listDistinctAudioApps` with the **same signatures** as their Linux counterparts in `audio/loopback-session.ts` / `audio/pipewire.ts`, so `ipc-handlers.ts` only needs a one-line platform branch to pick between them.

- [x] **Step 1: `audio/windows/process-identity.ts`** (deviation — added a name resolver)

  ```ts
  import { getPidForWindow } from '../../native/windows-audio';

  export function parseWindowsWindowId(sourceId: string): number | null {
    const match = sourceId.match(/^window:(\d+):/);
    return match ? parseInt(match[1], 10) : null;
  }

  export const resolveWindowHandle = getPidForWindow; // re-exported for symmetry with process-identity.ts
  ```

  (The exact HWND-parsing regex needs confirming against what Electron's
  `desktopCapturer` actually emits for `id` on Windows during Task 2's
  manual verification — expected to be the same `window:<id>:0` shape as
  Linux, since this format comes from Chromium itself rather than the OS,
  but confirm rather than assume.)

  **Deviation:** Step 2's "simpler" option (resolving a window's audio
  binary name up front, mirroring Linux) needs a PID→name resolver, which
  this file didn't originally export. Added `getExeNameForPid` as a new
  Task 2 native export (`process_identity::resolve_exe_name` made `pub`,
  wrapped as `#[napi] get_exe_name_for_pid`) and re-exported it here as
  `resolveExeNameForPid`, alongside `resolveWindowHandle`. Went with this
  option over "intersect with `list_active_audio_processes`" because that
  alternative only works when a window's own process is itself the one
  holding the audio session — exactly the case the plan's own
  "Verification risk" note says can't be assumed for Chromium apps.
  Import path is also `'../../../native/windows-audio/index.js'` (three
  levels up, with the explicit file), not the plan's `'../../native/
  windows-audio'` — this file lives at `desktop/src/audio/windows/`, and
  the module lives at `desktop/native/windows-audio/`, which is three
  `..`s away, not two; the explicit `index.js` avoids relying on
  cross-package `main`-field resolution for a relative specifier.

- [x] **Step 2: `audio/windows/loopback-session.ts`**

  Thin wrapper: `startAudioLoopback(target: AudioShareTarget)` calls the
  native `WindowsAudioSession.start(...)`, translating `AudioShareTarget`
  into the native `mode`/`target_name`/`excluded_names` shape (resolving
  the window target's binary name once via `list_active_audio_processes`
  intersected with the earlier-resolved PID, or — simpler — have
  `display-media-handler.ts` resolve the name once up front the same way
  it already resolves a binary name on Linux, so this module only ever
  receives a plain name string). The native `on_chunk` callback forwards
  each `Buffer` to the renderer: `mainWindow.webContents.send('desktop-audio-pcm-chunk', buffer)`.

  Went with "have `display-media-handler.ts` resolve the name up front"
  (see Step 1's deviation) — implemented in `resolveWindowsAudioTarget`
  there. Also converts each `Buffer` to a standalone `ArrayBuffer`
  (`toArrayBuffer`, slicing off `byteOffset`/`byteLength` from the
  possibly-pooled backing buffer) before sending, matching the plan's own
  Architecture section, which specifies `ArrayBuffer` — not `Buffer` — as
  the type that actually survives `contextBridge` cleanly. Also exports
  `listDistinctAudioApps` (mapping `listActiveAudioProcesses()`'s
  already-deduplicated `AudioProcessInfo[]` to `{ binary, label }[]`) —
  needed for `ipc-handlers.ts`'s `list-audio-apps` channel, which this
  step's text didn't mention but the Task's own **Interfaces** line and
  Step 3 both require.

- [x] **Step 3: Platform branch in `ipc-handlers.ts` and `display-media-handler.ts`** (both go further than the snippet — see below)

  ```ts
  const { startAudioLoopback, stopAudioLoopback } =
    process.platform === 'win32'
      ? await import('./windows/loopback-session.js')
      : await import('./loopback-session.js');
  ```

  Everything else in both files — the IPC channel names, the
  `AudioShareTarget` construction in `resolveAudioTarget`, the
  `list-audio-apps` handler's return shape — stays exactly as it is today.

  **Deviations, both load-bearing, not stylistic:**
  - `ipc-handlers.ts` also needs `listDistinctAudioApps`, which on Linux
    lives in a *separate* module (`pipewire.ts`), not
    `loopback-session.ts` — the Linux branch is
    `{ ...(await import('./loopback-session.js')), ...(await
    import('./pipewire.js')) }`, not a single import.
  - `display-media-handler.ts`'s own `resolveAudioTarget` is itself
    platform-specific (it calls `parseX11WindowId`/`resolveWindowPid`/
    `resolveProcessBinary` on Linux, the Windows equivalents from Step 1
    on Windows) — the one-liner above only covers
    `startAudioLoopback`/`stopAudioLoopback`, not this. Implemented by
    making `registerDisplayMediaHandler` itself `async`, resolving
    *both* the loopback-session module and (only when `isWindows`)
    `audio/windows/process-identity.ts` once via dynamic `import()` up
    front, then branching `resolveAudioTarget` between
    `resolveLinuxAudioTarget` (unchanged) and a new
    `resolveWindowsAudioTarget` closure. Both functions this module
    reaches for stay behind dynamic `import()`, never a static one —
    `native/windows-audio/index.js` (loaded transitively by
    `audio/windows/process-identity.ts` and `loopback-session.ts`)
    throws at module-evaluation time on any non-`win32` platform, so a
    static top-level import of anything under `audio/windows/` would
    crash the app immediately on Linux.
  - Both `registerAudioIpcHandlers` and `registerDisplayMediaHandler`
    becoming `async` (for the `await import(...)`) meant `main.ts` — not
    in this task's file list, but the only caller of either — needed a
    small change too: `app.whenReady().then(...)` now `await`s both
    calls, and `before-quit`'s `stopAudioLoopback()` call (previously a
    static import from the Linux-only `audio/loopback-session.js`) is
    now the same dynamic-import-by-platform pattern used everywhere
    else in `audio/`. Without this, quitting the app on Windows would
    never have stopped a running native audio session — real leaked
    capture threads/WASAPI handles on every quit while sharing, not a
    style nit.

- [x] **Step 4: `preload.ts` — the PCM bridge**

  ```ts
  contextBridge.exposeInMainWorld('desktopAudio', {
    start: (target: AudioShareTarget) => ipcRenderer.invoke('start-audio-loopback', target),
    stop: () => ipcRenderer.invoke('stop-audio-loopback'),
    onPcmChunk: (callback: (chunk: ArrayBuffer) => void) => {
      ipcRenderer.on('desktop-audio-pcm-chunk', (_event, chunk: ArrayBuffer) => callback(chunk));
    },
  });
  ```

  `onPcmChunk` only ever fires on Windows (nothing sends
  `desktop-audio-pcm-chunk` on Linux) — this is also exactly the signal
  Task 5 uses to pick which track-construction path to run.

- [x] **Step 5: Compile and sanity-check**

  ```bash
  cd desktop && pnpm exec tsc
  ```

  Ran for real: clean, zero errors, on Linux, with all of Steps 1-4's
  files in place. (One real type error surfaced and was fixed along the
  way: `Buffer.buffer` is typed `ArrayBufferLike`, not `ArrayBuffer`, so
  `toArrayBuffer` needed an explicit narrowing cast — safe here since a
  `Buffer` handed from a napi callback is never backed by a
  `SharedArrayBuffer`.)

  Expected: clean on Linux (the `windows/` modules are never imported
  there) and on Windows once Tasks 2–3 are in place.

  **Post-implementation code review (`/code-review low`) caught three
  real bugs in this task, all fixed:**
  - `preload.ts` exposed `onPcmChunk` unconditionally on every platform,
    but `webrtc.rs`'s `has_pcm_bridge()` (Task 5) treats that property's
    mere *existence* as its Windows-vs-Linux signal — so Linux would have
    silently taken the PCM-bridge path too (registering fine, since
    `onPcmChunk` is a real function there, just one nothing ever calls)
    and gotten a real but permanently silent audio track instead of ever
    trying its own working `getUserMedia` device-label path. Fixed by
    gating the property itself behind `process.platform === 'win32'` in
    `preload.ts`, not just behind whether it ever fires.
  - `main.ts`'s `before-quit` resolved the platform backend via a fresh
    `await import(...)` every time, un-awaited (Electron's `before-quit`
    doesn't wait for anything a listener returns or triggers) — a real
    race where the process could exit before that import even resolved,
    let alone before `stopAudioLoopback()` ran. Fixed by having
    `registerAudioIpcHandlers` (already awaited at startup, long before
    quit is possible) cache the resolved `stopAudioLoopback` in a
    module-level binding, exposed as a new synchronous
    `stopAudioLoopbackNow()` that `before-quit` calls directly.
  - `windowsIdentity.resolveWindowHandle(hwnd)`'s result was used as a
    PID (correctly, functionally) but named as if it returned a handle —
    renamed to `resolveWindowPid`, matching what it actually returns and
    the Linux function it's meant to mirror.

---

### Task 5: Browser-side track construction (Rust/wasm-bindgen)

> **Implemented 2026-08-26.** `cargo check`/`clippy` clean on both
> `--features ssr` and `--target wasm32-unknown-unknown --features
> hydrate`. One real bug caught and fixed before it ever ran (see Step 3):
> `AudioDataInit::new`'s `i32` timestamp parameter would have silently
> wrapped after ~35 minutes of continuous sharing.

**Files:**
- Modify: `src/ui/client/webrtc.rs`
- Modify: `Cargo.toml` (web-sys feature list)

**Interfaces:**
- Produces: `capture_display()` behaves identically from every caller's
  perspective; internally it picks between the existing Linux
  device-label `getUserMedia` path and this new one based on which bridge
  function `window.desktopAudio` actually exposes.

- [x] **Step 1: Add web-sys features** (three more than the plan expected — see below)

  Add to `Cargo.toml`'s existing web-sys feature list: `MediaStreamTrackGenerator`,
  `MediaStreamTrackGeneratorInit`, `AudioData`, `AudioDataInit`.

  Also added `AudioSampleFormat` (the enum `AudioDataInit`'s `format`
  field needs), `WritableStream`, `WritableStreamDefaultWriter` — see
  Step 3's deviation for why the latter two matter. This project's
  `.cargo/config.toml` already sets `--cfg=web_sys_unstable_apis` for the
  `wasm32-unknown-unknown` target (added earlier for Picture-in-Picture),
  which is what actually unlocks all of these — every one of them is
  `#[cfg(web_sys_unstable_apis)]`-gated in the real crate source. No
  config change was needed there; confirmed by it compiling immediately
  once the features were added.

- [x] **Step 2: Detect the Windows bridge**

  ```rust
  fn has_pcm_bridge() -> bool {
      let Some(window) = web_sys::window() else { return false };
      let Ok(desktop_audio) = js_sys::Reflect::get(&window, &JsValue::from_str("desktopAudio")) else { return false };
      js_sys::Reflect::has(&desktop_audio, &JsValue::from_str("onPcmChunk")).unwrap_or(false)
  }
  ```

  Implemented verbatim.

- [x] **Step 3: Build the track from incoming PCM chunks** (real `WritableStream` bindings exist — see below)

  Using the exact `format`/`timestamp` conventions written down in Task
  1: create one `MediaStreamTrackGenerator`, reflectively call
  `.writable.getWriter()` (no web-sys binding for `WritableStream` —
  confirmed in Task 1), and register a `Closure` against
  `window.desktopAudio.onPcmChunk` that, per incoming `ArrayBuffer`,
  constructs an `AudioData` (web-sys binding) with the agreed format and
  calls the writer's `.write()` reflectively. Keep the `Closure` alive
  (`.forget()`, matching this file's existing convention for long-lived
  listeners) and hold the generator/writer for the duration of the share.

  **Deviation, positive:** checked the actual installed `web-sys`
  0.3.104 source (not just Task 1's DevTools findings, which were about
  the JS API, not this crate) and found real, typed bindings for
  `WritableStream`/`WritableStreamDefaultWriter` — `MediaStreamTrackGenerator::writable()`
  returns a proper `WritableStream`, whose `.get_writer()` returns a
  proper `WritableStreamDefaultWriter` with a typed `write_with_chunk()`.
  Used those instead of `js_sys::Reflect` for this part — only
  `window.desktopAudio` itself (a hand-rolled object with no real DOM
  type) still needs `Reflect`, exactly where Task 1 already used it.

  **Deviation, a real bug avoided:** `AudioDataInit::new(..)`'s
  convenience constructor takes `timestamp: i32`, not `i64`/`f64`. At
  this mixer's cadence (a monotonically increasing microsecond counter,
  per Task 1's Step 5 findings), `i32` overflows after
  `2^31 / 1_000_000 ≈ 2147` seconds — **under 36 minutes** — silently
  wrapping the timestamp backwards for any share running longer than
  that, which is a completely ordinary screen-share duration. Built the
  `AudioDataInit` by hand instead (the same `unchecked_into(Object::new())`
  + field-setter pattern the convenience constructor uses internally,
  which is fully public API from outside the crate too) so
  `set_timestamp_f64` (an `f64` setter, also on the real struct) could be
  used instead. Caught by reading the actual generated bindings before
  writing the call site, not by testing — there was no way to exercise a
  36-minute share in this sandbox.

  Also wraps each `write_with_chunk()` promise in
  `wasm_bindgen_futures::spawn_local` rather than dropping it
  fire-and-forget: once the generator's track ends (e.g. after a later
  share stops), further writes reject, and an unobserved rejected
  promise would otherwise print an "Uncaught (in promise)" warning to
  the console on every subsequent chunk.

- [x] **Step 4: Wire it into `capture_display()`**

  ```rust
  pub async fn capture_display() -> Result<MediaStream, JsValue> {
      // ...unchanged: get the video_stream via getDisplayMedia...

      if has_pcm_bridge() {
          match build_track_from_pcm_bridge().await {
              Ok(audio_stream) => return combine_video_and_audio(&video_stream, &audio_stream),
              Err(_) => return Ok(video_stream),
          }
      }

      // ...unchanged: existing Linux getUserMedia device-label path...
  }
  ```

  Implemented essentially verbatim.

- [x] **Step 5: Compile-check both targets**

  ```bash
  cargo check --features ssr
  cargo check --target wasm32-unknown-unknown --features hydrate
  ```

  Ran for real (`--no-default-features` added to the `hydrate` invocation
  — this crate's features aren't additive-safe with the default feature
  set): clean on both, and `cargo clippy` + `cargo fmt` (scoped to
  `webrtc.rs` alone — the rest of the repo has pre-existing formatting
  drift under this sandbox's rustfmt 1.9.0 that predates this plan and is
  out of scope here) also clean.

  Expected: clean on both — this code only ever runs when
  `window.desktopAudio.onPcmChunk` exists, which is never true outside
  the Windows desktop app, but it still has to compile everywhere.

---

### Task 6: End-to-end manual verification (Windows machine required)

> **Status 2026-08-26: not started — needs the Windows VM.** Everything
> that can be built and checked from Linux (Tasks 1-5) is done: the
> native crate compiles and lints clean against the real `windows`/
> `wasapi`/`napi` APIs (via `--target x86_64-pc-windows-gnu`, since MSVC
> isn't available here), the TypeScript layer compiles clean
> (`pnpm exec tsc`), and the browser-side track construction compiles
> clean on both `ssr` and `hydrate`. None of that is a substitute for
> running on real Windows — nothing here has actually linked into a real
> `.node` binary, loaded it into Electron, or opened a real WASAPI
> session. **To build for real on the Windows VM:**
> 1. `cd desktop/native/windows-audio && npm install && npm run build`
>    (needs `@napi-rs/cli`, already a devDependency — this produces
>    `windows-audio.win32-x64-msvc.node`, which the hand-written
>    `index.js`/`index.d.ts` in this directory already know how to load;
>    no Rust code should need touching for this step to succeed, though
>    it's the actual first real compile against MSVC, so it might not).
> 2. `cd desktop && pnpm install && pnpm run build && pnpm start` (or
>    the project's usual desktop dev flow) to run the Electron app with
>    the native module in place.
> 3. Work through every checklist item below. Task 2 Step 5's and Task 3
>    Step 5's own manual-verification notes (empirical checks on process
>    identity and the raw mix) are worth doing first, in isolation,
>    before this full end-to-end pass — they're faster to iterate on and
>    will surface most native-layer bugs before involving the browser at
>    all.

Mirrors the Linux plan's own definition of done — check each of these
explicitly, since WASAPI's own quirks around session lifecycle/routing are
unknown territory and the Linux implementation hit two real,
non-obvious bugs (speaker duplication, stale node-name caching) that only
surfaced under actual use, not code review:

- [ ] Share a specific app's window with audio on — confirm only that
      app's audio reaches a second, real viewer.
- [ ] Share the whole screen with audio on and one app excluded — confirm
      everything else comes through and the excluded app doesn't.
- [ ] Start a new app making sound *after* sharing has already begun (both
      modes) — confirm it's picked up within about a second.
- [ ] While sharing with audio, confirm the sharer's own real speakers
      still work normally throughout (the Linux equivalent bug — a
      loopback stream unexpectedly duplicating onto the real output — is
      the single most important thing to explicitly check for here, since
      it's the one that actually broke someone's system audio last time).
- [ ] Watch your own share (or someone else's) on the same machine while
      also sharing with audio — confirm no feedback loop (the Linux
      self-exclusion bug's equivalent).
- [ ] Stop sharing — confirm every per-PID WASAPI capture and the native
      module's threads actually exit (no lingering capture holding a
      device open — check via Task 375, Resource Monitor's "Audio" tab,
      or by confirming the process's handle count drops).
- [ ] Share without checking the audio box at all — confirm behavior is
      unchanged (video-only, no errors).

## Definition of done

All of Task 6's manual checks pass on a real Windows 10 (2004+) or
Windows 11 machine. Audio sharing on Windows has full feature parity with
Linux: automatic single-process audio for window sharing, exclusion-list-
driven audio for screen sharing, dynamic mid-share pickup, and no
self-feedback — all behind the exact same `AudioShareTarget`/IPC surface
and picker UI the Linux implementation already shipped, with zero changes
to either.
