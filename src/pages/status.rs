/// Maps a human-readable status sentence to the signal lamp's visual state
/// (idle / busy / live / error) and a short eyebrow label.
///
/// Pure string classification instead of threading a separate state enum
/// through every call site — the status sentence is already the single
/// source of truth these components display.
pub fn status_meta(status: &str) -> (&'static str, &'static str) {
    match status {
        "Pronto para compartilhar." => ("idle", "PRONTO"),
        "Selecione a tela para compartilhar..." | "Conectando..." => ("busy", "CONECTANDO"),
        "Compartilhando! Envie o link para seus amigos." | "Conectado." => ("live", "AO VIVO"),
        "Compartilhamento encerrado." | "O compartilhamento foi encerrado." => {
            ("idle", "ENCERRADO")
        }
        s if s.starts_with("Sessão não encontrada") => ("error", "NÃO ENCONTRADA"),
        s if s.starts_with("Não foi possível")
            || s.starts_with("Seu navegador")
            || s.starts_with("Conexão perdida") =>
        {
            ("error", "ERRO")
        }
        _ => ("idle", "STATUS"),
    }
}
