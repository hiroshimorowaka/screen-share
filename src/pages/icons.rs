use leptos::prelude::*;

/// Ícones inline (traço simples, sem preenchimento — no estilo do
/// [Feather Icons](https://feathericons.com), MIT), usados nos botões de
/// ação da sala. Como funções puras (não componentes reativos), cada
/// chamada só monta a marcação; a cor vem de `currentColor`, herdando do
/// botão que a envolve.
/// Olho riscado — usado pro botão de "ocultar quem não está transmitindo"
/// no cabeçalho da sala (não confundir com `icon_screen_off`, que é sobre
/// parar de assistir UM compartilhamento específico).
pub fn icon_eye_off() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M17.94 17.94A10.94 10.94 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94"></path>
            <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19"></path>
            <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24"></path>
            <line x1="1" y1="1" x2="23" y2="23"></line>
        </svg>
    }
}

/// Monitor liso — "compartilhar minha tela" (a cor do botão que o envolve
/// já indica se está ativo ou não, igual ao resto dos toggles da barra).
pub fn icon_monitor() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="2" y="3" width="20" height="14" rx="2"></rect>
            <line x1="8" y1="21" x2="16" y2="21"></line>
            <line x1="12" y1="17" x2="12" y2="21"></line>
        </svg>
    }
}

/// Monitor com um X na tela — "parar de assistir esse compartilhamento".
pub fn icon_screen_off() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="2" y="3" width="20" height="14" rx="2"></rect>
            <line x1="8" y1="21" x2="16" y2="21"></line>
            <line x1="12" y1="17" x2="12" y2="21"></line>
            <line x1="9" y1="7" x2="15" y2="13"></line>
            <line x1="15" y1="7" x2="9" y2="13"></line>
        </svg>
    }
}

pub fn icon_eye() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path>
            <circle cx="12" cy="12" r="3"></circle>
        </svg>
    }
}

pub fn icon_log_out() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path>
            <polyline points="16 17 21 12 16 7"></polyline>
            <line x1="21" y1="12" x2="9" y2="12"></line>
        </svg>
    }
}

pub fn icon_maximize() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 3H5a2 2 0 0 0-2 2v3"></path>
            <path d="M21 8V5a2 2 0 0 0-2-2h-3"></path>
            <path d="M3 16v3a2 2 0 0 0 2 2h3"></path>
            <path d="M16 21h3a2 2 0 0 0 2-2v-3"></path>
        </svg>
    }
}

pub fn icon_minimize() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 14h6v6"></path>
            <path d="M20 10h-6V4"></path>
            <path d="M14 10l7-7"></path>
            <path d="M3 21l7-7"></path>
        </svg>
    }
}

/// Câmera — "mostrar meu preview" (estado atual: escondido).
pub fn icon_video() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>
            <path d="M23 7l-7 5 7 5V7z"></path>
        </svg>
    }
}

/// Câmera riscada — "esconder meu preview" (estado atual: visível).
pub fn icon_video_off() -> impl IntoView {
    view! {
        <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>
            <path d="M23 7l-7 5 7 5V7z"></path>
            <line x1="1" y1="1" x2="23" y2="23"></line>
        </svg>
    }
}
