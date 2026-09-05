//! The status-sentence classifier. Moved here with `status_kind` from
//! `apps/web/tests/ssr_render.rs` when the function moved into
//! `crates/domain`.

use screen_share_domain::status::{is_dismissible_error, status_kind, StatusKind};

#[test]
fn status_kind_classifies_the_known_idle_busy_and_live_sentences() {
    assert_eq!(status_kind(""), StatusKind::Idle);
    assert_eq!(status_kind("Pronto para criar uma sala."), StatusKind::Idle);
    assert_eq!(status_kind("Informe o nick da sala."), StatusKind::Idle);
    assert_eq!(status_kind("Conectando..."), StatusKind::Busy);
    assert_eq!(status_kind("Criando sala..."), StatusKind::Busy);
    assert_eq!(status_kind("Conectado."), StatusKind::Live);
}

#[test]
fn status_kind_treats_an_in_progress_reconnect_as_busy_not_an_error() {
    assert_eq!(
        status_kind("Reconectando... (tentativa 2 de 8)"),
        StatusKind::Busy
    );
    // But once retries are exhausted, the give-up sentence is an error.
    assert_eq!(
        status_kind("Conexão perdida. Recarregue a página para tentar de novo."),
        StatusKind::Error
    );
}

/// Every real validation/protocol failure message the app can set. Each
/// one used to render in the neutral "idle" color because it didn't match
/// any of the old classifier's hand-picked error prefixes — the bug this
/// module fixes by defaulting to `Error` instead of `Idle`.
#[test]
fn status_kind_classifies_every_known_error_sentence_as_an_error() {
    let known_errors = [
        "Nick vazio, muito longo ou com caracteres não permitidos.",
        "Nome da sala vazio, muito longo ou com caracteres não permitidos.",
        "Escolha uma cor da paleta.",
        "Digite uma senha ou marque \"sala pública\".",
        "Não foi possível conectar ao servidor.",
        "Preencha nick e senha.",
        "Sala não encontrada ou já foi encerrada.",
        "Senha incorreta.",
        "Essa sala já está cheia (máximo de 10 pessoas).",
        "Esta conexão já está em uma sala. Recarregue a página para entrar em outra.",
        "O servidor está sem capacidade no momento. Tente novamente em alguns minutos.",
        "Nick, nome da sala ou cor inválidos. Verifique e tente de novo.",
        "Muitas tentativas de senha erradas. Aguarde um pouco antes de tentar de novo.",
        "Você entrou nessa sala em outra aba ou janela — esta conexão foi encerrada.",
        "Conexão perdida. Recarregue a página para tentar de novo.",
    ];
    for message in known_errors {
        assert_eq!(
            status_kind(message),
            StatusKind::Error,
            "expected {message:?} to classify as an error"
        );
    }
}

#[test]
fn status_kind_defaults_unrecognized_text_to_error_not_idle() {
    // Anything not on the small idle/busy/live allow-list is presumed to
    // be a failure message a call site forgot to add here, not a harmless
    // one — see the doc comment on `status_kind`.
    assert_eq!(status_kind("mensagem qualquer"), StatusKind::Error);
}

#[test]
fn as_css_suffix_matches_the_lamp_and_status_text_css_modifiers() {
    assert_eq!(StatusKind::Idle.as_css_suffix(), "idle");
    assert_eq!(StatusKind::Busy.as_css_suffix(), "busy");
    assert_eq!(StatusKind::Live.as_css_suffix(), "live");
    assert_eq!(StatusKind::Error.as_css_suffix(), "error");
}

#[test]
fn ordinary_validation_and_protocol_errors_are_dismissible() {
    assert!(is_dismissible_error(
        "Nick vazio, muito longo ou com caracteres não permitidos."
    ));
    assert!(is_dismissible_error("Senha incorreta."));
    assert!(is_dismissible_error(
        "Essa sala já está cheia (máximo de 10 pessoas)."
    ));
}

#[test]
fn a_dead_connection_is_not_dismissible() {
    // Reverting these to a neutral prompt would claim the room works again
    // when it doesn't.
    assert!(!is_dismissible_error(
        "Conexão perdida. Recarregue a página para tentar de novo."
    ));
    assert!(!is_dismissible_error(
        "Você entrou nessa sala em outra aba ou janela — esta conexão foi encerrada."
    ));
}

#[test]
fn non_error_statuses_are_never_dismissible() {
    assert!(!is_dismissible_error(""));
    assert!(!is_dismissible_error("Conectando..."));
    assert!(!is_dismissible_error("Conectado."));
}
