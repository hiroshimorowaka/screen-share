#!/bin/sh
# Starts coturn in the background (only if TURN is actually configured for
# this deployment), then execs the app as PID 1 so it keeps receiving
# signals normally (SIGTERM on `fly deploy`/scale-to-zero). coturn ends up
# as PID 1's child either way — that's fine: it holds no state worth a
# graceful shutdown, so letting it die with the container on stop is an
# acceptable simplification for a single relay instance like this one, not
# a real supervisor.
set -e

# The app process (screen_share, below) validates TURN_SECRET at startup
# and aborts on a weak/placeholder value (finding F13) — coturn and the
# app read the same TURN_SECRET, so that check covers both. TURN_REALM
# should be set explicitly (see fly.toml); the :-screenshare fallback is
# only for a bare `docker run` locally.
if [ -n "$TURN_SECRET" ] && [ -n "$TURN_EXTERNAL_IP" ]; then
  # TURN_RELAY_IP: this machine's own private address (see the comment on
  # TURN_EXTERNAL_IP in fly.toml) — pins the actual relay sockets to one
  # real local interface. Deliberately NOT restricting --listening-ip to
  # the same address: Fly's edge delivers external traffic addressed to
  # 137.66.9.162 in a way that doesn't match this VM's private IP or
  # 127.0.0.1, so doing that made coturn unreachable from outside the VM
  # entirely (confirmed: external STUN requests timed out with zero
  # response once --listening-ip was added). Leaving the listening side on
  # its default (every discovered local address) is what actually works;
  # the stray "socket: Protocol not supported" lines at startup are for
  # addresses coturn can't bind (IPv6 ones this VM doesn't support) and are
  # harmless.
  # --denied-peer-ip: coturn will refuse to relay packets toward these
  # destinations. Without it, anyone holding a (freely handed out) TURN
  # credential can make the relay reach the cloud metadata endpoint
  # (169.254.169.254), the Fly 6PN private network (fdaa::/16, inside
  # fc00::/7) and any RFC1918/CGNAT host — an SSRF vantage point on the
  # infrastructure. Recent coturn denies loopback by default but NOT
  # link-local or private ranges, so they are listed explicitly. Legit
  # media peers are public browsers, so denying private space costs
  # nothing.
  #
  # Quotas/bandwidth caps stop the same credential from turning the relay
  # into a traffic amplifier billed to this account: --total-quota /
  # --user-quota bound concurrent allocations, --max-bps bounds one
  # allocation's throughput, --bps-capacity bounds the whole server's.
  # Defaults are generous for a 10-member room yet finite; override via
  # env if a deployment needs more.
  #
  # TLS (finding F16): the media itself is always SRTP-encrypted, but the
  # STUN/TURN control channel is plaintext unless a cert is supplied. Set
  # TURN_TLS_CERT and TURN_TLS_KEY (paths to a mounted cert/key for
  # TURN_REALM's hostname) to also listen for `turns:` on 5349 — add the
  # matching `turns:` URL to TURN_URLS and open port 5349 in fly.toml.
  # Without them coturn stays plaintext-only, as before.
  if [ -n "$TURN_TLS_CERT" ] && [ -n "$TURN_TLS_KEY" ]; then
    tls_args="--tls-listening-port=${TURN_TLS_PORT:-5349} --cert=$TURN_TLS_CERT --pkey=$TURN_TLS_KEY"
  else
    tls_args="--no-tls --no-dtls"
  fi

  # shellcheck disable=SC2086 # tls_args is an intentional word-split list
  turnserver \
    --no-cli \
    --fingerprint \
    --use-auth-secret \
    --static-auth-secret="$TURN_SECRET" \
    --realm="${TURN_REALM:-screenshare}" \
    --listening-port=3478 \
    --min-port="${TURN_MIN_PORT:-49160}" \
    --max-port="${TURN_MAX_PORT:-49300}" \
    --external-ip="$TURN_EXTERNAL_IP" \
    ${TURN_RELAY_IP:+--relay-ip="$TURN_RELAY_IP"} \
    --denied-peer-ip=0.0.0.0-0.255.255.255 \
    --denied-peer-ip=10.0.0.0-10.255.255.255 \
    --denied-peer-ip=100.64.0.0-100.127.255.255 \
    --denied-peer-ip=169.254.0.0-169.254.255.255 \
    --denied-peer-ip=172.16.0.0-172.31.255.255 \
    --denied-peer-ip=192.168.0.0-192.168.255.255 \
    --denied-peer-ip=::1 \
    --denied-peer-ip=fc00::-fdff:ffff:ffff:ffff:ffff:ffff:ffff:ffff \
    --denied-peer-ip=fe80::-febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff \
    --no-multicast-peers \
    --total-quota="${TURN_TOTAL_QUOTA:-300}" \
    --user-quota="${TURN_USER_QUOTA:-12}" \
    --max-bps="${TURN_MAX_BPS:-2000000}" \
    --bps-capacity="${TURN_BPS_CAPACITY:-40000000}" \
    --log-file=stdout \
    $tls_args &
fi

exec ./screen_share
