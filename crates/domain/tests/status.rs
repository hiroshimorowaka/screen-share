//! The status-sentence classifier. Moved here with `status_meta` from
//! `apps/web/tests/ssr_render.rs` when the function moved into
//! `crates/domain`.

use screen_share_domain::status::status_meta;

#[test]
fn status_meta_classifies_the_known_sentences() {
    assert_eq!(status_meta("Pronto para compartilhar."), ("idle", "PRONTO"));
    assert_eq!(status_meta("Conectando..."), ("busy", "CONECTANDO"));
    assert_eq!(status_meta("Conectado."), ("live", "AO VIVO"));
    assert_eq!(
        status_meta("O compartilhamento foi encerrado."),
        ("idle", "ENCERRADO")
    );
}

#[test]
fn status_meta_treats_an_in_progress_reconnect_as_busy_not_an_error() {
    assert_eq!(
        status_meta("Reconectando... (tentativa 2 de 8)"),
        ("busy", "RECONECTANDO")
    );
    // But once the retries are exhausted, the give-up sentence is an error.
    assert_eq!(
        status_meta("Conexão perdida. Recarregue a página para tentar de novo."),
        ("error", "ERRO")
    );
}

#[test]
fn status_meta_classifies_error_sentences_by_prefix() {
    assert_eq!(
        status_meta("Sala não encontrada: XYZ"),
        ("error", "NÃO ENCONTRADA")
    );
    assert_eq!(
        status_meta("Seu navegador não suporta..."),
        ("error", "ERRO")
    );
    assert_eq!(
        status_meta("Conexão perdida, tentando..."),
        ("error", "ERRO")
    );
    assert_eq!(
        status_meta("Você entrou nessa sala em outra aba."),
        ("error", "DESCONECTADO")
    );
}

#[test]
fn status_meta_falls_back_to_a_neutral_status_for_anything_else() {
    assert_eq!(status_meta("mensagem qualquer"), ("idle", "STATUS"));
}
