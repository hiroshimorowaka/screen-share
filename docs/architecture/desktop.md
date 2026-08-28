# Desktop

The desktop app is an Electron shell around the same web app. Electron is
an execution platform, not the screenshare logic.

## Target structure (after refactor Phase 6)

```
desktop/src/
├── main/         Electron lifecycle, windows, tray
├── features/
│   ├── screen-share/   picker, getDisplayMedia handler, share wiring
│   └── audio-share/    service that selects a backend, loopback session
├── ipc/          channel names, handlers, payload types
└── platform/
    ├── windows/  WASAPI loopback via native/windows-audio, process filter
    └── linux/    PipeWire loopback, process filter
```

`process.platform` branching lives only in
`features/audio-share/service.ts`, which picks a `platform/windows` or
`platform/linux` `AudioBackend` implementation once at startup. The rest
of the code depends on the `AudioBackend` interface.

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
