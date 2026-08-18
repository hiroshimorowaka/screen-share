use leptos::prelude::*;

use crate::pages::status::status_meta;

#[component]
pub fn HomePage() -> impl IntoView {
    let (status, set_status) = signal("Pronto para compartilhar.".to_string());
    let (room_link, set_room_link) = signal(None::<String>);
    let (copied, set_copied) = signal(false);
    let (is_sharing, set_is_sharing) = signal(false);
    let supported = display_media_supported();

    let toggle_sharing = sharing_toggle_handler(set_status, set_room_link, is_sharing, set_is_sharing);
    let copy_link = copy_link_handler(room_link, set_copied);

    let lamp_class = move || {
        let (variant, _) = status_meta(&status.get());
        format!("lamp lamp--{variant}")
    };
    let eyebrow_label = move || status_meta(&status.get()).1;
    let button_class = move || {
        if is_sharing.get() {
            "btn btn--danger"
        } else {
            "btn btn--primary"
        }
    };

    view! {
        <div class="panel">
            <div class="status-row">
                <span class=lamp_class></span>
                <span class="eyebrow">{eyebrow_label}</span>
            </div>

            <h1>"Compartilhar tela"</h1>
            <p class="subtext">"Escolha uma janela ou tela e mande o link pra quem quiser ver."</p>

            <Show when=move || !supported>
                <p class="status-text status-text--error">
                    "Seu navegador não suporta compartilhamento de tela. Tente um navegador atualizado (Chrome, Edge, Firefox)."
                </p>
            </Show>

            <button class=button_class on:click=toggle_sharing disabled=move || !supported>
                {move || if is_sharing.get() { "Parar compartilhamento" } else { "Iniciar compartilhamento" }}
            </button>

            <Show when=move || supported>
                <p class="status-text" class:status-text--error=move || status_meta(&status.get()).0 == "error">
                    {status}
                </p>
            </Show>

            <Show when=move || room_link.get().is_some()>
                <div class="invite">
                    <p class="invite__label">"Link da sala"</p>
                    <div class="invite__row">
                        <a class="invite__link" href=move || room_link.get().unwrap_or_default()>
                            {move || room_link.get().unwrap_or_default()}
                        </a>
                        <button class="btn btn--ghost" on:click=copy_link.clone()>
                            {move || if copied.get() { "Copiado!" } else { "Copiar" }}
                        </button>
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[cfg(not(feature = "hydrate"))]
fn display_media_supported() -> bool {
    true
}

#[cfg(feature = "hydrate")]
fn display_media_supported() -> bool {
    crate::client::webrtc::is_display_media_supported()
}

#[cfg(not(feature = "hydrate"))]
fn copy_link_handler(
    _room_link: ReadSignal<Option<String>>,
    _set_copied: WriteSignal<bool>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
fn copy_link_handler(
    room_link: ReadSignal<Option<String>>,
    set_copied: WriteSignal<bool>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use wasm_bindgen::JsCast;

    move |_| {
        let Some(link) = room_link.get_untracked() else { return };
        let Some(window) = web_sys::window() else { return };
        let _ = window.navigator().clipboard().write_text(&link);

        set_copied.set(true);
        let reset = wasm_bindgen::prelude::Closure::once_into_js(move || {
            set_copied.set(false);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            reset.as_ref().unchecked_ref(),
            1500,
        );
    }
}

#[cfg(not(feature = "hydrate"))]
fn sharing_toggle_handler(
    set_status: WriteSignal<String>,
    _set_room_link: WriteSignal<Option<String>>,
    _is_sharing: ReadSignal<bool>,
    _set_is_sharing: WriteSignal<bool>,
) -> impl Fn(leptos::ev::MouseEvent) + 'static {
    move |_| {
        set_status.set("Pronto para compartilhar.".to_string());
    }
}

#[cfg(feature = "hydrate")]
fn sharing_toggle_handler(
    set_status: WriteSignal<String>,
    set_room_link: WriteSignal<Option<String>>,
    is_sharing: ReadSignal<bool>,
    set_is_sharing: WriteSignal<bool>,
) -> impl Fn(leptos::ev::MouseEvent) + 'static {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::{MediaStream, MediaStreamTrack, RtcPeerConnection, RtcPeerConnectionIceEvent};

    use crate::client::socket::WsClient;
    use crate::client::webrtc::{add_ice_candidate, capture_display, create_offer, new_peer_connection};
    use crate::signaling::protocol::{ClientMessage, ServerMessage};

    let ws_slot: Rc<RefCell<Option<WsClient>>> = Rc::new(RefCell::new(None));
    let peers: Rc<RefCell<HashMap<String, RtcPeerConnection>>> = Rc::new(RefCell::new(HashMap::new()));
    let local_stream: Rc<RefCell<Option<MediaStream>>> = Rc::new(RefCell::new(None));

    // Usado tanto quando o navegador avisa que a captura terminou (botão nativo
    // "Stop sharing") quanto quando a pessoa clica em "Parar compartilhamento"
    // no nosso próprio botão — as duas situações precisam do mesmo cleanup.
    fn stop_sharing(
        ws_slot: &Rc<RefCell<Option<WsClient>>>,
        peers: &Rc<RefCell<HashMap<String, RtcPeerConnection>>>,
        local_stream: &Rc<RefCell<Option<MediaStream>>>,
        set_room_link: WriteSignal<Option<String>>,
        set_is_sharing: WriteSignal<bool>,
        set_status: WriteSignal<String>,
        final_status: &str,
    ) {
        if let Some(stream) = local_stream.borrow_mut().take() {
            for track in stream.get_tracks().iter() {
                let track: MediaStreamTrack = track.unchecked_into();
                track.stop();
            }
        }
        if let Some(ws) = ws_slot.borrow_mut().take() {
            ws.close();
        }
        for (_, pc) in peers.borrow_mut().drain() {
            pc.close();
        }
        set_room_link.set(None);
        set_is_sharing.set(false);
        set_status.set(final_status.to_string());
    }

    move |_| {
        if is_sharing.get_untracked() {
            stop_sharing(
                &ws_slot,
                &peers,
                &local_stream,
                set_room_link,
                set_is_sharing,
                set_status,
                "Pronto para compartilhar.",
            );
            return;
        }

        let ws_slot = ws_slot.clone();
        let peers = peers.clone();
        let local_stream = local_stream.clone();

        set_status.set("Selecione a tela para compartilhar...".to_string());

        spawn_local(async move {
            let stream = match capture_display().await {
                Ok(stream) => stream,
                Err(_) => {
                    set_status.set("Pronto para compartilhar.".to_string());
                    return;
                }
            };
            *local_stream.borrow_mut() = Some(stream);
            set_is_sharing.set(true);
            set_status.set("Conectando...".to_string());

            // O navegador também expõe seu próprio botão "Stop sharing" na barra
            // de captura — sem isso, clicar nele deixava quem assiste com a
            // última imagem congelada, porque nunca avisávamos o servidor.
            if let Some(stream_ref) = local_stream.borrow().as_ref() {
                if let Some(track) = stream_ref.get_tracks().get(0).dyn_into::<MediaStreamTrack>().ok() {
                    let ws_for_end = ws_slot.clone();
                    let peers_for_end = peers.clone();
                    let local_stream_for_end = local_stream.clone();
                    let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                        stop_sharing(
                            &ws_for_end,
                            &peers_for_end,
                            &local_stream_for_end,
                            set_room_link,
                            set_is_sharing,
                            set_status,
                            "Compartilhamento encerrado.",
                        );
                    });
                    track.set_onended(Some(onended.as_ref().unchecked_ref()));
                    onended.forget();
                }
            }

            let ws_slot_for_messages = ws_slot.clone();
            let peers_for_messages = peers.clone();
            let local_stream_for_messages = local_stream.clone();

            let on_message = move |msg: ServerMessage| {
                let ws_slot = ws_slot_for_messages.clone();
                let peers = peers_for_messages.clone();
                let local_stream = local_stream_for_messages.clone();

                match msg {
                    ServerMessage::RoomCreated { room, peer_id: _ } => {
                        let origin = web_sys::window().unwrap().location().origin().unwrap();
                        set_room_link.set(Some(format!("{origin}/r/{room}")));
                        set_status.set("Compartilhando! Envie o link para seus amigos.".to_string());
                    }
                    ServerMessage::PeerJoined { peer_id } => {
                        spawn_local(async move {
                            let Ok(pc) = new_peer_connection() else { return };

                            // Guarda a conexão já aqui, antes de negociar. Candidatos
                            // ICE do espectador podem chegar a qualquer momento a partir
                            // de agora; o navegador só sabe enfileirá-los até a resposta
                            // (answer) ser aplicada se já houver uma RTCPeerConnection
                            // para receber. Guardar isso só no fim (como estava antes)
                            // fazia esses candidatos serem descartados no braço
                            // `IceCandidate` mais abaixo, deixando a transmissão sem
                            // rota de mídia utilizável (tela preta do outro lado).
                            peers.borrow_mut().insert(peer_id.clone(), pc.clone());

                            if let Some(stream) = local_stream.borrow().as_ref() {
                                for track in stream.get_tracks().iter() {
                                    let track: web_sys::MediaStreamTrack = track.unchecked_into();
                                    pc.add_track_0(&track, stream);
                                }
                            }

                            let target_id = peer_id.clone();
                            let ws_for_ice = ws_slot.clone();
                            let onicecandidate = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcPeerConnectionIceEvent)>::new(
                                move |event: RtcPeerConnectionIceEvent| {
                                    if let Some(candidate) = event.candidate() {
                                        if let Some(ws) = ws_for_ice.borrow().as_ref() {
                                            ws.send(&ClientMessage::IceCandidate {
                                                to: target_id.clone(),
                                                candidate: candidate.candidate(),
                                                sdp_mid: candidate.sdp_mid(),
                                                sdp_m_line_index: candidate.sdp_m_line_index(),
                                            });
                                        }
                                    }
                                },
                            );
                            pc.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));
                            onicecandidate.forget();

                            let failed_peer_id = peer_id.clone();
                            let oniceconnectionstatechange = {
                                let pc_for_state = pc.clone();
                                wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                                    if pc_for_state.ice_connection_state() == web_sys::RtcIceConnectionState::Failed {
                                        set_status.set(format!(
                                            "Não foi possível conectar com um espectador ({failed_peer_id})."
                                        ));
                                    }
                                })
                            };
                            pc.set_oniceconnectionstatechange(Some(oniceconnectionstatechange.as_ref().unchecked_ref()));
                            oniceconnectionstatechange.forget();

                            if let Ok(sdp) = create_offer(&pc).await {
                                if let Some(ws) = ws_slot.borrow().as_ref() {
                                    ws.send(&ClientMessage::Offer { to: peer_id, sdp });
                                }
                            }
                        });
                    }
                    ServerMessage::Answer { from, sdp } => {
                        if let Some(pc) = peers.borrow().get(&from).cloned() {
                            spawn_local(async move {
                                let _ = crate::client::webrtc::accept_answer(&pc, &sdp).await;
                            });
                        }
                    }
                    ServerMessage::IceCandidate { from, candidate, sdp_mid, sdp_m_line_index } => {
                        if let Some(pc) = peers.borrow().get(&from) {
                            add_ice_candidate(pc, &candidate, sdp_mid, sdp_m_line_index);
                        }
                    }
                    ServerMessage::PeerLeft { peer_id } => {
                        if let Some(pc) = peers.borrow_mut().remove(&peer_id) {
                            pc.close();
                        }
                    }
                    _ => {}
                }
            };

            match WsClient::connect("/ws", on_message) {
                Ok(ws) => {
                    ws.on_open({
                        let ws_slot = ws_slot.clone();
                        move || {
                            if let Some(ws) = ws_slot.borrow().as_ref() {
                                ws.send(&ClientMessage::CreateRoom);
                            }
                        }
                    });
                    *ws_slot.borrow_mut() = Some(ws);
                }
                Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
            }
        });
    }
}
