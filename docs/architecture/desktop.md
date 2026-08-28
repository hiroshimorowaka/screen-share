# Desktop

The desktop app is an Electron shell around the same web app. Electron is
an execution platform, not the screenshare logic.

## Structure (refactor Phase 6)

```
desktop/src/
├── preload.ts                    contextBridge — kept at src root so it
│                                 compiles to dist/preload.js
├── main/
│   ├── index.ts                  app entry (package.json "main")
│   ├── lifecycle.ts              quitting flag
│   ├── window.ts                 the main BrowserWindow + startQuickShare
│   └── tray.ts
├── features/
│   ├── screen-share/
│   │   ├── display-media.ts      setDisplayMediaRequestHandler
│   │   ├── picker.ts             the source-picker BrowserWindow
│   │   └── quick-share.ts        the tray quick-share IPC (link + notify)
│   └── audio-share/
│       └── ipc.ts                start/stop/list IPC handlers
├── ipc/
│   └── types.ts                  PickerSource / PickerChoice / ShareChoice /
│                                 AudioShareTarget (import-type only, so
│                                 the sandboxed preload can use it)
└── platform/
    ├── run-command.ts            spawn → stdout helper
    ├── linux/
    │   ├── pipewire.ts           pw-loopback / pw-link mix
    │   ├── loopback.ts           the Linux audio loopback session
    │   └── process-identity.ts   /proc + xprop window→binary
    └── windows/
        ├── audio.ts              WASAPI loopback via native/windows-audio
        └── process-identity.ts   hwnd→pid→exe via native/windows-audio
```

`process.platform` branching is confined to two dynamic-import switches —
`features/audio-share/ipc.ts` and `features/screen-share/display-media.ts`
— each picking `platform/windows/*` or `platform/linux/*` at startup so a
Linux process never even evaluates `native/windows-audio/index.js` (which
throws on load off win32/x64). Folding both into one
`features/audio-share/backend.ts` `AudioBackend` loader is a follow-up
(Phase 6b).

Channel-name strings stay as literals in both `preload.ts` and its main
-process counterpart (with "must match … exactly" comments) rather than a
shared `channels.ts` — the sandboxed preload can only `import type`, so a
shared runtime constants module would not reach it.

`__dirname`-relative runtime paths after the renest: `main/window.ts` and
`features/screen-share/picker.ts` load `../…/preload.js`; `main/tray.ts`
loads `../../icons/tray-icon.png`; `picker.ts` loads
`../../../static/picker.html`. `tsc` does not check these — they are
verified by launching the app.

## Native audio boundary

```
Electron → AudioService → WindowsAudioBackend → napi → Rust → WASAPI
                        → PipeWireAudioBackend → PipeWire
```

`desktop/native/windows-audio` is a standalone napi crate (its own
`Cargo.lock`, built by `electron-builder`), deliberately **not** a member
of the Rust workspace — it is Windows-only and would complicate feature
unification.

See [ADR-0004](../decisions/0004-desktop-electron-and-windows-native-audio.md).
