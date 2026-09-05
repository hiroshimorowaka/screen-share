//! The status-sentence classifier for the web app's status-driven UI.
//! The pt-BR strings are user-facing copy matched against the pt-BR status
//! sentences the home and room pages set — domain copy, not framework code
//! (see CLAUDE.md §1: English for code, Portuguese for user-facing
//! strings).

/// The visual state a status sentence maps to — drives the connection lamp
/// color and whether the sentence renders in the error (red) style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Nothing in flight, nothing wrong — an initial prompt or a settled,
    /// non-error rest state.
    Idle,
    /// An action is in flight (connecting, creating a room, reconnecting).
    Busy,
    /// Actively connected and sharing/watching.
    Live,
    /// A validation failure, a protocol error, or a connection failure.
    Error,
}

impl StatusKind {
    /// The CSS class modifier for this state, e.g. `lamp--{suffix}`.
    #[must_use]
    pub fn as_css_suffix(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Busy => "busy",
            Self::Live => "live",
            Self::Error => "error",
        }
    }
}

/// Classifies the status sentence directly, without a separate state enum
/// threaded through every `set_status` call site — the sentence itself is
/// already the single source of truth shown by the components.
///
/// Unrecognized text classifies as [`StatusKind::Error`], not
/// [`StatusKind::Idle`]. A status sentence is only ever set to report
/// progress or a known-good outcome, so anything that isn't one of the
/// small set of idle/busy/live sentences below is far more likely to be a
/// failure message than a harmless one — and defaulting to idle let
/// several real errors (a too-long nick, a wrong password, a full room...)
/// render in the neutral color and sit on screen indefinitely, since the
/// call site that reports them has no way to also update this list. See
/// `docs/decisions/0010-error-status-default.md`.
#[must_use]
pub fn status_kind(status: &str) -> StatusKind {
    match status {
        "" | "Pronto para criar uma sala." | "Informe o nick da sala." => StatusKind::Idle,
        "Conectando..." | "Criando sala..." => StatusKind::Busy,
        s if s.starts_with("Reconectando") => StatusKind::Busy,
        "Conectado." => StatusKind::Live,
        _ => StatusKind::Error,
    }
}

/// Whether an error status should auto-revert to a neutral prompt after a
/// delay, instead of sitting on screen forever. `false` for the reconnect
/// give-up and "kicked" sentences: both describe a connection that is
/// actually dead, so reverting them to a cheerful idle prompt would claim
/// the room works again when it doesn't — those stay until the visitor
/// reloads or leaves. `false` for anything that isn't an error at all.
#[must_use]
pub fn is_dismissible_error(status: &str) -> bool {
    status_kind(status) == StatusKind::Error
        && !status.starts_with("Conexão perdida")
        && !status.starts_with("Você entrou nessa sala em outra")
}
