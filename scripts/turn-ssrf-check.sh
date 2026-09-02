#!/usr/bin/env bash
# Manual verification for the coturn peer-IP denylist (findings F01 / F03,
# docs/decisions/0008-security-hardening.md). There is no in-repo coturn
# test harness — this is the "verification is manual" step those sections
# refer to. Run it from a machine OUTSIDE the Fly VM (a laptop), against
# the deployed relay.
#
# What it does: mints a short-lived TURN REST credential from the shared
# secret (the same scheme crates/signaling/src/turn.rs uses), then asks
# the relay to open a permission toward a series of addresses that MUST be
# refused — the cloud metadata endpoint, RFC1918 / CGNAT / link-local
# ranges, loopback, and the IPv4-mapped-IPv6 forms of the same
# (::ffff:169.254.169.254, ...). coturn answers a denied peer with STUN
# error 403 (Forbidden). A public control address is tried last and must
# NOT get a 403.
#
# PASS  = every forbidden address was refused (403) and the control was not.
# FAIL  = the relay created a permission toward a forbidden address — the
#         denylist has a hole; do not ship.
#
# Requirements: turnutils_uclient (ships with coturn: `apt install coturn`
# or `brew install coturn`), plus openssl and xxd for the HMAC.
#
# Usage:
#   TURN_SECRET=... scripts/turn-ssrf-check.sh [turn-host] [turn-port]
#
#   turn-host defaults to the TURN_EXTERNAL_IP in fly.toml; port to 3478.
#   TURN_SECRET must match what `fly secrets` holds for the deployment.

set -u

TURN_HOST="${1:-137.66.9.162}"
TURN_PORT="${2:-3478}"

if [ -z "${TURN_SECRET:-}" ]; then
  echo "error: set TURN_SECRET (must match the deployed relay's static-auth-secret)" >&2
  exit 2
fi
for bin in turnutils_uclient openssl; do
  command -v "$bin" >/dev/null 2>&1 || { echo "error: $bin not found on PATH" >&2; exit 2; }
done

# REST credential: username = a Unix timestamp the credential expires at,
# password = base64(HMAC-SHA1(username, secret)). Mirrors mint_credentials.
USERNAME="$(( $(date +%s) + 3600 ))"
PASSWORD="$(printf '%s' "$USERNAME" \
  | openssl dgst -sha1 -hmac "$TURN_SECRET" -binary \
  | openssl base64)"

# Addresses that must be refused. Keep in sync with the --denied-peer-ip
# list in docker-entrypoint.sh.
FORBIDDEN=(
  169.254.169.254          # cloud metadata
  127.0.0.1                # loopback (the app + coturn itself)
  10.0.0.1                 # RFC1918
  172.16.0.1               # RFC1918
  192.168.0.1             # RFC1918
  100.64.0.1              # CGNAT
  ::ffff:169.254.169.254  # IPv4-mapped IPv6 metadata (F03 bypass)
  ::ffff:127.0.0.1        # IPv4-mapped IPv6 loopback
)
CONTROL="1.1.1.1"          # public — a permission here is expected to succeed

run_probe() { # $1 = peer address -> prints the turnutils output
  timeout 20 turnutils_uclient \
    -y -c -s -n 2 -m 1 \
    -e "$1" \
    -u "$USERNAME" -w "$PASSWORD" \
    -p "$TURN_PORT" "$TURN_HOST" 2>&1
}

denied() { grep -qiE '403|forbidden|Permission[^:]*error|cannot create permission'; }

echo "relay: $TURN_HOST:$TURN_PORT   credential exp: $USERNAME"
echo

fail=0
for ip in "${FORBIDDEN[@]}"; do
  out="$(run_probe "$ip")"
  if printf '%s\n' "$out" | denied; then
    printf '  PASS  %-24s refused (403)\n' "$ip"
  else
    printf '  FAIL  %-24s NOT refused — denylist hole\n' "$ip"
    printf '%s\n' "$out" | sed 's/^/          | /'
    fail=1
  fi
done

out="$(run_probe "$CONTROL")"
if printf '%s\n' "$out" | denied; then
  printf '  WARN  %-24s a public peer was refused — denylist too broad\n' "$CONTROL"
  fail=1
else
  printf '  PASS  %-24s permission allowed (control)\n' "$CONTROL"
fi

echo
if [ "$fail" -eq 0 ]; then
  echo "RESULT: PASS — the relay refuses every private / metadata peer."
else
  echo "RESULT: FAIL — see the lines above."
fi
exit "$fail"
