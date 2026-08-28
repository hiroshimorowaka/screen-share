#!/bin/sh
# Starts coturn in the background (only if TURN is actually configured for
# this deployment), then execs the app as PID 1 so it keeps receiving
# signals normally (SIGTERM on `fly deploy`/scale-to-zero). coturn ends up
# as PID 1's child either way — that's fine: it holds no state worth a
# graceful shutdown, so letting it die with the container on stop is an
# acceptable simplification for a single relay instance like this one, not
# a real supervisor.
set -e

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
    --log-file=stdout \
    --no-tls \
    --no-dtls &
fi

exec ./screen_share
