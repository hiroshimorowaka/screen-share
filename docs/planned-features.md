# Planned features

Candidate features for this project, ranked easiest → hardest to
implement given our stack (in-memory registry, P2P WebRTC via `web-sys`,
Electron desktop wrapper). Audio capture is out of scope here — tracked
separately.

## 1. Password attempt rate limiting

Backend-only, contained in `signaling/auth.rs` / `registry.rs`. Counter per
room/IP with a time window; no protocol or WebRTC changes. Matches the
existing unit-test style in those modules.

## 2. Native notification on member join

Electron `Notification` API, wired to the existing "member joined"
signaling event. Low effort.

## 3. Embedded TURN server

Doesn't change the P2P architecture — just adds a TURN server (self-hosted
coturn or a managed service) to the ICE server list in `webrtc.rs`. The
real cost is operational (standing up/maintaining coturn), not code
complexity. Addresses a real gap: without TURN, peers behind CGNAT or
restrictive firewalls can't connect at all.

## 4. Optional public rooms (no password)

Small registry change (`public: bool` flag, skip password check when set).
Conflicts with the current product invariant ("rooms are always
password-protected", per `CLAUDE.md`) — needs a deliberate product
decision before implementation, even though the code change itself is
small.

## 5. Adaptive video quality (bitrate/resolution control)

Doesn't require full simulcast — each connection is already an independent
P2P peer connection, so bitrate can be capped per-peer via
`RTCRtpSender.setParameters()` after negotiation. Needs UI to expose the
control.

## 6. Vanity room names

Needs an availability-check + validation flow, similar to the existing
room-code generation, but a new surface on the registry.

## 7. Viewer roster with connection telemetry (direct vs. relay)

Needs a `getStats()` polling loop, a new signaling message to report ICE
candidate type, and UI to display it. Touches the shared signaling
protocol, so higher effort than the items above.

## 8. Custom capture picker with thumbnails + live source switching

Meaningful Electron UI work (thumbnails via `desktopCapturer`) plus
`replaceVideoTrack` wiring on the WebRTC side to switch source without
dropping the call. We already have `picker.ts` as a starting point, but
thumbnails + live switching is a bigger scope than what's there today.

---

**Suggested starting point:** #1 (PIN rate limiting) — isolated, testable
in the same style as the existing auth/registry tests, no product-design
questions attached, and it's the one real security gap identified so far.
