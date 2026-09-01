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
