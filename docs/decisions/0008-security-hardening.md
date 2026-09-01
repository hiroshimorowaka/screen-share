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
