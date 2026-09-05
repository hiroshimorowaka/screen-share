# 0010 — Status classifier defaults to error, and errors are dismissible

Status: accepted
Date: 2026-09-05

## Context

The status-driven UI (`CLAUDE.md` §"Status-driven UI") classifies one
human-readable status sentence into `idle` / `busy` / `live` / `error`
with `screen_share_domain::status::status_meta` (now `status_kind`), an
allow-list matched against known sentence prefixes: anything not
recognized fell through to `idle`.

That allow-list drifted out of sync with the sentences the app actually
sets. A user-reported bug (a too-long nick) showed the real symptom: the
validation error rendered in the neutral "tip" color, not red, and then
sat on screen indefinitely — nothing ever reverted it. Auditing every
`set_status`/`fixed_status_text` call site found this wasn't an isolated
miss: of ~15 distinct error sentences the app can set, only 3 matched an
existing error prefix. The rest — a too-long nick or room name, a missing
password, a wrong password, a full room, a rate limit, a bad protocol
message — rendered as plain "idle" text. Two of the classifier's own
prefixes (`"Seu navegador"`, `"Compartilhamento encerrado."` /
`"O compartilhamento foi encerrado."` / `"Pronto para compartilhar."`)
matched sentences no call site produces any more.

## Decision

**Default to error, not idle.** `status_kind` now allow-lists the small,
closed set of genuinely non-error sentences (the two idle prompts, the two
busy/in-flight sentences, the one "live" sentence) and classifies
everything else — known or not yet written — as `StatusKind::Error`. A
future call site that sets a new failure sentence gets the red styling for
free instead of silently rendering as neutral until someone remembers to
extend an allow-list. The dead prefixes above were removed along with
their now-passing tests.

**Dismissible errors auto-revert.** Most errors here are one-off blips
from a single form submission or protocol response — the visitor reads
the message, then either retries or leaves the field alone; showing it
forever was actively confusing, not just cosmetic. `is_dismissible_error`
marks an error as safe to auto-revert to a neutral prompt after a fixed
delay (`apps/web/src/client/dom::auto_dismiss_error`, wired from
`home::page` and `room::page`), *except* the reconnect give-up
("Conexão perdida...") and "kicked" sentences: those describe a
connection that is actually dead, and reverting them to a cheerful idle
prompt would claim the room works again when it doesn't. Those two stay
on screen until the visitor reloads or leaves.

## Consequences

- `status_kind` returns a `StatusKind` enum instead of a
  `(&str, &str) `tuple; the unused short-label half of the old tuple
  (never rendered anywhere) was dropped rather than ported.
- `apps/web`'s three status-bearing signals (home create panel, home join
  panel, room gate/stage) each get one `auto_dismiss_error` call at page
  setup; each panel already owns the neutral sentence to revert to (its
  signal's initial value), now named `INITIAL_STATUS` so the two copies
  can't drift apart the way the classifier's allow-list did.
