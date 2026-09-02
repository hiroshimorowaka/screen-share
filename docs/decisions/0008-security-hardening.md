# 0008 — Security hardening sweep (audit P1–P3)

Status: accepted
Date: 2026-09-01

## Coverage

All 19 findings (F01–F19) addressed. Two carry a residual that is ops, not
code, and is called out in the relevant section:

- **F04 / F16** — need a certificate provisioned in CI / on the relay; the
  code and config paths are in place and inert until then.
- **F12** — the CSP must be smoke-tested in a real browser before it is
  trusted; that was not possible in the environment this work was done in.

F14 is a documented risk acceptance (the audit offers this, gated on the
F12 CSP being in place).

## Context

An external security audit (`docs/security-audit/`) produced 19 findings
(F01–F19) across the signaling relay, the TURN infrastructure, the Axum
HTTP surface, and the Electron desktop app. This ADR records the decisions
taken while working through the P1–P3 recommendations so the rationale
isn't buried in commit messages.

Nothing here changes the product's shape (a persistent room, decoupled
share/watch, P2P media). Every change is defence-in-depth or an abuse
limit.

## Decisions

### F01 — coturn peer-IP allowlist and quotas

`docker-entrypoint.sh` now starts `turnserver` with:

- `--denied-peer-ip` ranges covering `0.0.0.0/8`, `10/8`, `100.64/10`
  (CGNAT), `169.254/16` (link-local / cloud metadata), `172.16/12`,
  `192.168/16`, `::ffff:0:0/96` (IPv4-mapped IPv6 — see below), IPv6
  `::1`, `fc00::/7` (ULA, contains Fly's `fdaa::/16` 6PN) and `fe80::/10`.
  Legit media peers are public browsers, so denying private space costs
  nothing and removes the relay as an SSRF vantage point onto the cloud
  metadata endpoint and the internal network.
- The `::ffff:0.0.0.0-::ffff:255.255.255.255` range was added in the
  2026-09-02 follow-up pass: the shipped coturn is 4.6.2, and on
  coturn < 4.9.0 an IPv4-mapped IPv6 peer address (`::ffff:127.0.0.1`,
  `::ffff:169.254.169.254`, ...) bypasses every IPv4 `--denied-peer-ip`
  rule. Denying the mapped range closes that bypass; it stays as defence
  in depth once the base image ships coturn ≥ 4.9.0.
- `--no-multicast-peers`.
- `--total-quota` (300), `--user-quota` (12), `--max-bps` (2 MB/s per
  allocation), `--bps-capacity` (40 MB/s server-wide), each overridable
  via `TURN_TOTAL_QUOTA` / `TURN_USER_QUOTA` / `TURN_MAX_BPS` /
  `TURN_BPS_CAPACITY`. Bounds how much traffic one freely-minted
  credential can push through the relay on this account's bill.

`CREDENTIAL_TTL` in `crates/signaling/src/turn.rs` dropped from 6h to 1h:
a fresh credential is minted on every `Joined` (every reconnect), so 1h
still covers a session while shrinking the window a credential lifted
from a `Joined` snapshot stays useful.

Verification is manual (`turnutils_uclient` against `169.254.169.254` and
an RFC1918 address must fail; `--max-bps` visible in the running
process) — coturn has no in-repo test harness. The TTL change is covered
by `turn_tests.rs`.

### F02 / F03 — signaling relay resource limits and connection-bind guards

The relay had no ceiling on anything an unauthenticated socket could
drive. Added, all as named `const`s in `signaling`:

- `MAX_MESSAGE_BYTES` (256 KiB) via `WebSocketUpgrade::max_message_size` —
  caps a text frame before axum-ws buffers up to its ~64 MiB default.
- `IDLE_TIMEOUT` (90 s) around each `ws_receiver.next()` — reaps a
  slowloris socket that connects and goes silent. The client pings every
  5 s once joined, so this is far clear of a healthy connection.
- `RATE_WINDOW` (10 s) / `MAX_MSGS_PER_WINDOW` (300) sliding-window
  message-rate cap (pure `over_rate_limit` helper, unit-tested); a flood
  loop trips it and the socket is closed.
- `MAX_WS_CONNECTIONS` (2 000) via `Registry::try_acquire_connection`
  returning an RAII `ConnectionGuard`; the upgrade is refused with 503
  once at the cap.
- `MAX_ROOMS` (5 000) global and `MAX_ROOMS_PER_CLIENT` (50, keyed by the
  `client_key` IP header) enforced in `create_room`, which is now
  fallible (`CreateRoomError::AtCapacity` -> `ServerMessage::ServerAtCapacity`).
- The per-connection outbound channel is now **bounded**
  (`OUTBOUND_CAPACITY` = 256, `mpsc::channel`); registry broadcasts use
  `try_send`, so a member that stops reading its socket gets a hard
  memory ceiling (messages drop) instead of an unbounded backlog. A
  wedged peer connection was already broken; the trade is deliberate.

F03: `CreateRoom`/`JoinRoom` on a socket that already holds a room are
refused with `ServerMessage::AlreadyInRoom` instead of silently
overwriting the connection's `room_code`/`peer_id` and leaking the first
room's membership forever. One `peer_id` per connection is now an
invariant.

New wire variants: `ServerMessage::AlreadyInRoom`,
`ServerMessage::ServerAtCapacity` (round-tripped in `protocol` tests;
surfaced as a status line in the web client).

### F04 / F18 — desktop auto-update integrity

`desktop/package.json`:

- `build.win.verifyUpdateCodeSignature` was `false`, which told the NSIS
  updater to skip the publisher-signature check — trust in an update came
  down to "it was HTTPS from our GitHub". Now `true`: an update whose
  installer isn't Authenticode-signed by the same publisher as the
  running app is refused. A signed release build is therefore required
  for Windows auto-update to *apply* — CI must supply `CSC_LINK` /
  `CSC_KEY_PASSWORD`; an unsigned build still runs and simply never
  self-updates (the safe failure mode). `updates.ts`'s doc block records
  this.
- `build.asar` was `false` (F18) — the packaged app's files sat loose on
  disk with no integrity boundary. Now `true`, which required two
  follow-ons found by building the packages:
  - `build.asarUnpack` for `**/*.node` — a native addon can't be
    `require`d from inside an asar archive, so the Windows `windows-audio`
    binding must be unpacked or system-audio capture breaks on Windows.
  - `build.linux.maintainer` set to a GitHub noreply alias — dropping
    `author.email` for F19 left the `.deb` target with no Debian
    `Maintainer`; electron-builder needs one there. The alias is
    non-routable, so it satisfies F19's "project/alias, not personal".

Both flags have no runtime code surface, so the regression guard is a
vitest test (`packaging-security.test.ts`) asserting the manifest keeps
these values; the `.node` / maintainer bits are verified by
`pnpm dist:linux` actually producing both packages.

Not done here (ops, not code): provisioning the Windows signing
certificate in CI. Until that lands, packaged Windows auto-update is
inert by design rather than insecure.

### F05 / F09 — `/ws` handshake: `Origin` allowlist and trusted-proxy client key

New module `signaling::handshake` (`HandshakeConfig`, `OriginPolicy`),
read once from the environment and carried in `SignalingState`:

- **F05** — `SIGNALING_ALLOWED_ORIGINS` (comma-separated). When set, a
  `/ws` upgrade whose `Origin` isn't on the list gets `403` before any
  signaling runs; a request with no `Origin` (native clients) still
  passes. Unset ⇒ `AllowAll` (local dev, and any deployment that hasn't
  opted in). `fly.toml` sets it to the app's own origin. This is defence
  in depth, not auth.
- **F09** — `client_key` (the wrong-password lockout / per-client room
  cap key) no longer blindly trusts `fly-client-ip`. It's used only when
  `TRUST_PROXY_HEADERS` is truthy (set in `fly.toml`, since Fly's edge
  overwrites the header); otherwise, and whenever the header is absent,
  the real TCP peer address is used. The old `"unknown"` shared-constant
  fallback is gone — off-Fly, a client can no longer rotate a spoofed
  header to escape the lockout, nor can an absent header collapse every
  client onto one bucket and lock a room for everyone.

`ws_handler` now also extracts `ConnectInfo<SocketAddr>`, so the server is
served with `into_make_service_with_connect_info`.

Covered by `handshake_tests.rs` (parsing, origin decisions, client-key
selection) and a `signaling_ws` integration test (cross-origin handshake
refused, app origin accepted).

### F06 — `GET /api/rooms/:code` minimised and rate limited

`room_status_handler` no longer returns `name` or `member_count` — only
`{ exists, requires_password }`, the minimum for the dead-link check and
the password-field decision. The human-chosen room name and the occupancy
were an information leak to anyone holding a code and made enumeration
observable; the name is still delivered in the `Joined` snapshot once a
client is actually in the room.

Added a process-wide per-client sliding-window rate limiter
(`RoomStatusLimiter`, 30 requests / 10 s, keyed via the same
`HandshakeConfig::client_key`), returning `429` past the budget, with a
bounded tracking map. Unit-tested in `rooms_status_tests.rs`; the `429`
path has an integration test.

Client fallout (accepted, and the now-dead client paths were removed):
the pre-join screen shows only the room code until the `Joined` snapshot
arrives with the name, and the home page dropped its per-room "N/10"
occupancy badge. The liveness fetch still runs — it now feeds a plain
"which remembered rooms are up" set that drives pruning and the lobby
readout ("N salas recentes no ar"), with no count. The `RoomStatus` wire
shape still carries `name`/`member_count` (always `None` from this
endpoint); no client code reads them.

### F07 — peer-to-peer signaling requires a watch relationship

The relay isolated by room but never checked that the two ends had agreed
to connect, so any co-member could send `Offer { to: victim }` and make
the victim's browser open an `RTCPeerConnection`, trickle ICE (revealing
its LAN + public address), and answer — plus spam renegotiation and
`SetQuality`.

`Registry::relay` became `relay_peer_signal(room, from, to, msg)` and
forwards only when `watch_related(room, from, to)` — one of the two is in
the other's `watchers` set (direction-agnostic: `Offer` flows
sharer→viewer, `Answer`/half the ICE flow back). The legit flow already
runs `WatchShare` (→ `add_watcher`) before the first `Offer`, so it's
unaffected. The web client also ignores an `Offer` from a peer not in its
`watching` set, as defence in depth.

Covered by a `signaling_ws` test (unsolicited `Offer` dropped, socket
still live) and updated relay tests that now establish the watch first.

### F10 / F11 — desktop renderer navigation lock and IPC sender checks

The main `BrowserWindow` loads a remote origin and exposes powerful
bridges (`desktopAudio`, `picker`, `desktopShare`). Nothing stopped that
renderer being navigated elsewhere, and no IPC handler checked where a
message came from.

- **F10** — `window.ts` pins `contextIsolation` / `sandbox` /
  `nodeIntegration:false` / `webSecurity` explicitly, and `lockNavigation`
  blocks `will-navigate` / `will-redirect` off the app origin and denies
  `window.open` (real links go to the OS browser via `shell.openExternal`).
  SPA routing (`pushState`) and the main-process `loadURL` in
  `startQuickShare` don't trip these, so nothing legitimate is blocked.
  The picker window gets pinned prefs and a deny-all open handler too.
- **F11** — new `main/ipc-guard.ts` `isTrustedFrame(event)`: an IPC
  message is honoured only from a frame on the app origin or a local
  `file://` frame (the picker). Applied to every `ipcMain` handler —
  `start`/`stop`/`list` audio (throw on reject), `desktop-share:link-ready`
  / `member-joined` / `sharing-changed` and `picker:selected` (ignore on
  reject). A hijacked/XSS'd remote page in the renderer can no longer
  start a covert system-audio capture, enumerate running apps, hijack the
  clipboard, or spoof OS notifications.

`main/app-url.ts` now owns `APP_URL` / `APP_ORIGIN` for both `window.ts`
and the guard. Covered by `ipc-guard.test.ts`, a navigation-lock test in
`window.test.ts`, and untrusted-sender cases added to the audio and
quick-share IPC tests.

### F08 / F15 — protocol input validation and registry state guards

New serde-only `protocol::validate` module (`crates/protocol` stays
dependency-free): `MAX_NICK_LEN` (32), `MAX_ROOM_NAME_LEN` (64),
`PALETTE_IDS` + `DEFAULT_COLOR`, and `clean_nick` / `clean_room_name` /
`is_valid_color`. `clean_name` trims, rejects empty / over-length (counted
in characters), and rejects any control character or bidi / zero-width
formatting character (RLO, isolates, ZWSP, LRM/RLM, BOM, …). Full Unicode
NFC normalisation is deliberately skipped — it needs a dependency; the
character rejection removes the impersonation vectors the audit cited.

The relay enforces these in `create_room` / `join_room`
(`CreateRoomError::InvalidInput` / `JoinError::InvalidInput` →
`ServerMessage::InvalidInput`), so an oversized nick is never stored or
rebroadcast and an off-palette colour is refused rather than
silently defaulted. The web create form mirrors the checks; a
`palette_tests` assertion keeps the render palette and the allowlist from
drifting.

F15, in `registry`:
- `add_watcher` ignores a `sharer_id` that isn't a room member, so a
  client can't pollute the `watchers` map or trigger spurious
  `WatchersChanged` broadcasts.
- `report_latency` drops a value above `MAX_PLAUSIBLE_LATENCY_MS`
  (60 s) instead of rebroadcasting it as that peer's ping.

Covered by `protocol/tests/validate.rs`, a `wire` round-trip for
`InvalidInput`, and three new `registry` tests.

### F12 — HTTP security headers

New `apps/web` `http_security` module (SSR only) — an
`axum::middleware::from_fn` layer that stamps every response with
`Content-Security-Policy`, `Strict-Transport-Security` (2 y),
`Permissions-Policy` (`display-capture=(self)`; `camera` / `microphone` at
`self` — `=()` there makes Chrome log a policy violation on every
`getDisplayMedia({ audio: true })`; `geolocation` denied),
`X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
`X-Frame-Options: DENY`, `Cross-Origin-Opener-Policy: same-origin`.

The CSP is intentionally loose where the stack needs it:
`'wasm-unsafe-eval'` for the wasm-bindgen module; `script-src
'unsafe-inline'` because `leptos_meta::HydrationScripts` emits a
nonce-less inline bootstrap `<script>` (the `leptos` `nonce` feature is
off) that a stricter `script-src` would block, leaving the page
un-hydrated — moving this to a per-request nonce is a follow-up;
`style-src 'unsafe-inline'` for Leptos's `style="--member: …"` bindings;
the Google Fonts hosts. Everything else is `'self'`. Still worth a
real-browser smoke test (create / join / watch, no CSP violations
logged); that wasn't possible in this environment.
`tests/http_security.rs` asserts the header set and the CSP's required
directives.

Non-PROD runs (`cargo leptos watch`) get a variant that also allows
plaintext `ws:` in `connect-src` — the dev server's live-reload socket
runs on a second `ws://` port that the production policy correctly
blocks. Selected from `leptos_options.env` at startup; production is
unaffected.

### F13 — reject a weak `TURN_SECRET` at startup

`TurnConfig::from_vars` now returns `Result<Option<Self>, TurnConfigError>`
instead of `Option<Self>`. Both vars unset ⇒ `Ok(None)` (STUN-only, still
valid). Both set ⇒ the secret is checked against
`MIN_TURN_SECRET_LEN` (24) and a placeholder denylist
(`changeme`, `secret`, `screenshare`, …); a bad one is `Err`, and
`main.rs` propagates it so the process aborts rather than run a relay
anyone can mint credentials for. `TURN_REALM` is now set explicitly in
`fly.toml` instead of relying on the public `screenshare` default; the
`docker-entrypoint.sh` fallback stays only for a bare local `docker run`.
Covered by `turn_tests.rs`.

### F14 — room password in `sessionStorage`

Accepted, not replaced (the audit offers this explicitly, gated on CSP).
`RoomSession.password` is still persisted so a same-tab reload rejoins
silently; `sessionStorage` is tab-scoped and auto-clearing. The mitigation
is the CSP added in F12 (keeps injected script off the origin) plus the
desktop `senderFrame` guard (F11). The `RoomSession` doc comment records
the tradeoff and points at this ADR; a short-lived server-minted rejoin
token is the escalation path if an XSS foothold on the origin ever becomes
plausible.

### F16 — TURN control channel over TLS

The media is always SRTP-encrypted; only the STUN/TURN control channel was
plaintext. `docker-entrypoint.sh` now adds a `turns:` listener on 5349
when `TURN_TLS_CERT` / `TURN_TLS_KEY` are provided (paths to a mounted
cert/key), and stays `--no-tls --no-dtls` otherwise. Actually closing F16
needs a certificate provisioned for the relay hostname, the matching
`turns:` URL in `TURN_URLS`, and port 5349 opened in `fly.toml` — an ops
task; the mechanism is now in place.

## Follow-up remediation (2026-09-02)

A second manual audit pass (`.handoff/security-and-leak-remediation.md`)
found further defence-in-depth gaps and client/desktop resource leaks.
Numbering below is that document's (Finding 1–13); the P1/P2 items are
addressed here, P3 is deferred.

### Finding 2 — bounded argon2 cost + password length cap

`Argon2::default()` asks for 19 MiB per call; joining needs no prior auth,
so a burst of wrong-password `JoinRoom`s could OOM the 256 MB VM.
`crates/signaling/src/auth.rs` now builds an explicit Argon2id hasher at
OWASP's lowest-memory profile — `ARGON2_MEMORY_KIB` 7168 (7 MiB),
`ARGON2_ITERATIONS` 5, `ARGON2_PARALLELISM` 1 — equivalent brute-force
resistance to the 19 MiB / t=2 default at ~2.7x less memory. Argon2 embeds
its parameters in the PHC string, so hashes written with the old cost keep
verifying (`tests/auth.rs`). `MAX_PASSWORD_LEN` (128) is validated in
`create_room` / `join_room` before hashing; longer is `InvalidInput`.

### Finding 3 — coturn `::ffff:` bypass

Covered inline in the F01 section above.

### Finding 1 — room session teardown on every exit, not just the button

`RoomPage`'s `on_cleanup` (`drop_peers_on_cleanup`) closed the peer
connections but left `conn.ws` open, `expected_close` false, and the
reconnect loop armed. On any non-button exit (browser back, `navigate`,
an SPA route change) the server's 90 s idle reap then tripped `on_close`,
which read the still-present `sessionStorage` credentials and rejoined
the room on a page the user had left — every ~90 s, forever, spamming
`PeerJoined`/`PeerLeft` at the real members. The `conn.ws` ->
message-closure -> `RoomSession` clone -> `conn.ws` `Rc` cycle also kept
the whole session alive for the tab's life.

`session::reconnect::teardown_session(conn)` now centralises the
non-navigation teardown: `expected_close = true`, `reconnecting = false`,
drop the peer connections, and **take** the `WsClient` out of its
`RefCell` and close it (taking it out is what breaks the cycle).
`drop_peers_on_cleanup` and the "leave" button (`watch::leave_room`) both
call it, so every exit runs one teardown path. `WsClient` also got a
`Drop` impl that closes its socket as a backstop.

### Finding 8a — native "Stop sharing" listener no longer leaked per share

`attach_native_stop_listener` did `onended.forget()`, leaking the closure
(and the `RoomSession` clone it captures — which also anchored the
Finding 1 cycle) once per `start_sharing` **and** once per source switch.
The closure is now held in a single `RoomSession.local_capture_callback`
slot that `teardown_local_share` clears and a new capture replaces; the
outgoing closure is dropped on the microtask queue, not synchronously,
because on a source switch the listener being replaced is the one
currently running.

### Finding 8b — desktop PCM-bridge listener no longer leaked per share

`build_track_from_pcm_bridge` (`infra::webrtc`, Windows desktop only)
`forget()`'d the `onPcmChunk` closure that owns the
`WritableStreamDefaultWriter` + `MediaStreamTrackGenerator`. It is now
held in a module `thread_local` single slot that a fresh bridge replaces
and `stop_desktop_audio_loopback` clears once the native side has stopped
emitting chunks.

### Finding 9 — `AudioContext` released on the probe's error paths

`audio_health::listen_for_sound` only called `ctx.close()` on the success
path; every `?` after `AudioContext::new()` returned `Err` with the
context still open, and a browser caps a page at ~6 — after a handful of
failed probes (one per share-start / source-switch) the probe was dead
for the session. The graph is now built in an inner `run_probe(&ctx, …)`
and `close()` runs unconditionally afterwards.

### Finding 4 — `failed_password_attempts` map no longer grows unbounded

`password_attempts_exceeded` (registry) swept only the caller's own key,
so a slow distributed brute force (one attempt per source IP, keys never
revisited) leaked one entry per IP forever. It now sweeps every key on
each call and drops the ones whose sliding window has emptied.

### Finding 5 — non-Text WebSocket frames now count against the rate limit

`ws.rs` extracted `Message::Text` before the `over_rate_limit` check, so
Binary/Ping/Pong frames (each up to `MAX_MESSAGE_BYTES`, and a ping is
answered with a pong out) were bounded only by `IDLE_TIMEOUT` and the
connection cap. The rate check now runs on every frame; a binary frame —
never valid for this JSON-text protocol — closes the connection.

### Finding 10 — `fly-client-ip` only trusted from an internal TCP peer

`HandshakeConfig::client_key` honoured `fly-client-ip` under
`TRUST_PROXY_HEADERS` without checking the TCP peer. It now requires the
peer to be loopback or in a private / link-local range (`is_internal_peer`)
— Fly delivers every edge request from its internal network, so a public
peer means the header is attacker-controlled and the real peer IP is used
instead.

### Finding 11 — empty `device_id` no longer evicts another empty one

`join_room`'s duplicate-device removal now skips entirely when
`device_id` is empty (the web client's `ensure_device_id` returns `""` on
every failure path), so two locked-down browsers joining a public room no
longer kick each other.

### Finding 6 — Electron permission handler

The desktop renderer loads a remote origin and had no
`setPermissionRequestHandler` / `setPermissionCheckHandler`, so a
compromised app origin could prompt for `getUserMedia`, geolocation,
notifications, etc. New `main/permissions.ts` (`lockDownPermissions`,
called from `whenReady`) denies every request and every check. Screen
capture is unaffected — it goes through `setDisplayMediaRequestHandler`.

### Finding 7 — audio loopback torn down when the renderer goes away

`audioSession` / `activeSession` were only cleared by an explicit
`stop-audio-loopback`, the mix process exiting, or `before-quit`. A
quick-share `loadURL`, a reload, or a renderer crash orphaned
`pw-loopback` / the WASAPI capture, its 1 s poll and its `pw-link`s (and
on Windows kept `desktop-audio-pcm-chunk` firing at a dead frame).
`window.ts` now calls `stopAudioLoopbackNow()` on the main
`webContents`'s `did-start-navigation` (main frame, cross-document),
`destroyed`, and `render-process-gone`.

### Finding 8c / 8d — accumulating IPC listeners

- `preload.ts`: `picker.onSources` switched to `ipcRenderer.once`
  (`picker:sources` is sent once per window); `desktopAudio.onPcmChunk`
  now `removeAllListeners` before re-adding (it runs in the persistent
  main-window preload, once per Windows share) and exposes `offPcmChunk`.
- `screen-share/picker.ts`: the `ipcMain.once('picker:selected', …)`
  handler is captured in a named const and `removeListener`'d in
  `settle`, so a dismissed (never-selected) picker doesn't leak one
  `ipcMain` listener per cancellation.

### Finding 13 — `isTrustedFrame` and picker window hardening

`isTrustedFrame` trusted **any** `file://` URL. It now matches the
picker's exact file URL (`PICKER_FILE_URL`, shared via the new
`features/screen-share/picker-page.ts`). The picker `BrowserWindow` also
gains `devTools: !app.isPackaged` and the main window's `lockNavigation`
(`will-navigate` / `will-redirect` / `window.open` guards), which
`window.ts` now exports.
