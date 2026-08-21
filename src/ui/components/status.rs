/// Classifica a frase de status direto, sem um enum de estado separado — a
/// frase já é a fonte única de verdade exibida pelos componentes.
pub fn status_meta(status: &str) -> (&'static str, &'static str) {
    match status {
        "Pronto para compartilhar." => ("idle", "PRONTO"),
        "Selecione a tela para compartilhar..." | "Conectando..." => ("busy", "CONECTANDO"),
        "Compartilhando! Envie o link para seus amigos." | "Conectado." => ("live", "AO VIVO"),
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
