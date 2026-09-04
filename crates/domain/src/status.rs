//! The status-sentence classifier for the web app's status-driven UI.
//! The pt-BR strings are user-facing copy matched against the pt-BR status
//! sentences the room page sets — domain copy, not framework code (see
//! CLAUDE.md §1: English for code, Portuguese for user-facing strings).

/// Classifies the status sentence directly, without a separate state enum —
/// the sentence itself is already the single source of truth shown by the
/// components. Returns `(visual_state, short_label)` where `visual_state`
/// is one of `"idle"` / `"busy"` / `"live"` / `"error"`.
pub fn status_meta(status: &str) -> (&'static str, &'static str) {
    match status {
        "Pronto para compartilhar." => ("idle", "PRONTO"),
        "Conectando..." => ("busy", "CONECTANDO"),
        s if s.starts_with("Reconectando") => ("busy", "RECONECTANDO"),
        "Conectado." => ("live", "AO VIVO"),
        "Compartilhamento encerrado." | "O compartilhamento foi encerrado." => {
            ("idle", "ENCERRADO")
        }
        s if s.starts_with("Sala não encontrada") => ("error", "NÃO ENCONTRADA"),
        s if s.starts_with("Não foi possível")
            || s.starts_with("Seu navegador")
            || s.starts_with("Conexão perdida") =>
        {
            ("error", "ERRO")
        }
        s if s.starts_with("Você entrou nessa sala em outra") => ("error", "DESCONECTADO"),
        _ => ("idle", "STATUS"),
    }
}
