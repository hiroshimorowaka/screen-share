# 0008 — Security hardening sweep (audit P1–P3)

Status: in progress
Date: 2026-09-01

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
  `192.168/16`, IPv6 `::1`, `fc00::/7` (ULA, contains Fly's `fdaa::/16`
  6PN) and `fe80::/10`. Legit media peers are public browsers, so denying
  private space costs nothing and removes the relay as an SSRF vantage
  point onto the cloud metadata endpoint and the internal network.
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
  disk with no integrity boundary. Now `true`.

Both flags have no runtime code surface, so the regression guard is a
vitest test (`packaging-security.test.ts`) asserting the manifest keeps
these values.

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

Client fallout (acceptable): the room page shows no name until joined,
and the home page's "N/10" badge on remembered rooms no longer appears
(the fetch still runs for the liveness/pruning check). The `RoomStatus`
wire shape keeps `name`/`member_count` as always-`None` fields for
compatibility.

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
