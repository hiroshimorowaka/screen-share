use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    let (status, set_status) = signal("Pronto para compartilhar.".to_string());
    let (room_link, set_room_link) = signal(None::<String>);

    let start_sharing = start_sharing_handler(set_status, set_room_link);

    view! {
        <div class="home">
            <h1>"Compartilhar tela"</h1>
            <button on:click=start_sharing>"Iniciar compartilhamento"</button>
            <p>{status}</p>
            <Show when=move || room_link.get().is_some()>
                <p>
                    "Link para convidar: "
                    <a href=move || room_link.get().unwrap_or_default()>
                        {move || room_link.get().unwrap_or_default()}
                    </a>
                </p>
            </Show>
        </div>
    }
}

#[cfg(not(feature = "hydrate"))]
fn start_sharing_handler(
    set_status: WriteSignal<String>,
    _set_room_link: WriteSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + 'static {
    move |_| {
        set_status.set("Pronto para compartilhar.".to_string());
    }
}

#[cfg(feature = "hydrate")]
fn start_sharing_handler(
    set_status: WriteSignal<String>,
    set_room_link: WriteSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + 'static {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::{MediaStream, RtcPeerConnection, RtcPeerConnectionIceEvent};

    use crate::client::socket::WsClient;
    use crate::client::webrtc::{add_ice_candidate, capture_display, create_offer, new_peer_connection};
    use crate::signaling::protocol::{ClientMessage, ServerMessage};

    let ws_slot: Rc<RefCell<Option<WsClient>>> = Rc::new(RefCell::new(None));
    let peers: Rc<RefCell<HashMap<String, RtcPeerConnection>>> = Rc::new(RefCell::new(HashMap::new()));
    let local_stream: Rc<RefCell<Option<MediaStream>>> = Rc::new(RefCell::new(None));

    move |_| {
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
            set_status.set("Conectando...".to_string());

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

                            if let Ok(sdp) = create_offer(&pc).await {
                                if let Some(ws) = ws_slot.borrow().as_ref() {
                                    ws.send(&ClientMessage::Offer { to: peer_id.clone(), sdp });
                                }
                            }

                            peers.borrow_mut().insert(peer_id, pc);
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
