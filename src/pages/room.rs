use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[component]
pub fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let code = move || params.read().get("code").unwrap_or_default();
    let initial_code = params.read_untracked().get("code").unwrap_or_default();

    let (status, set_status) = signal("Conectando...".to_string());
    let video_ref = NodeRef::<leptos::html::Video>::new();

    setup_viewer_connection(initial_code, set_status, video_ref);

    view! {
        <div class="room">
            <h1>"Assistindo sala " {code}</h1>
            <p>{status}</p>
            <video node_ref=video_ref autoplay=true playsinline=true style="max-width: 100%;"></video>
        </div>
    }
}

#[cfg(not(feature = "hydrate"))]
fn setup_viewer_connection(
    _room_code: String,
    _set_status: WriteSignal<String>,
    _video_ref: NodeRef<leptos::html::Video>,
) {
}

#[cfg(feature = "hydrate")]
fn setup_viewer_connection(
    room_code: String,
    set_status: WriteSignal<String>,
    video_ref: NodeRef<leptos::html::Video>,
) {
    use std::cell::RefCell;
    use std::rc::Rc;

    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::{MediaStream, RtcPeerConnection, RtcPeerConnectionIceEvent, RtcTrackEvent};

    use crate::client::socket::WsClient;
    use crate::client::webrtc::{add_ice_candidate, create_answer, new_peer_connection};
    use crate::signaling::protocol::{ClientMessage, ServerMessage};

    let ws_slot: Rc<RefCell<Option<WsClient>>> = Rc::new(RefCell::new(None));
    let pc_slot: Rc<RefCell<Option<RtcPeerConnection>>> = Rc::new(RefCell::new(None));

    let on_message = {
        let ws_slot = ws_slot.clone();
        let pc_slot = pc_slot.clone();
        move |msg: ServerMessage| {
            let ws_slot = ws_slot.clone();
            let pc_slot = pc_slot.clone();

            match msg {
                ServerMessage::RoomNotFound => {
                    set_status.set("Sessão não encontrada ou já terminou.".to_string());
                }
                ServerMessage::Offer { from, sdp } => {
                    spawn_local(async move {
                        let Ok(pc) = new_peer_connection() else { return };

                        let ontrack = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcTrackEvent)>::new(
                            move |event: RtcTrackEvent| {
                                if let Ok(stream) = event.streams().get(0).dyn_into::<MediaStream>() {
                                    if let Some(video) = video_ref.get() {
                                        video.set_src_object(Some(&stream));
                                    }
                                }
                            },
                        );
                        pc.set_ontrack(Some(ontrack.as_ref().unchecked_ref()));
                        ontrack.forget();

                        let target_id = from.clone();
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

                        let oniceconnectionstatechange = {
                            let pc_for_state = pc.clone();
                            wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                                if pc_for_state.ice_connection_state() == web_sys::RtcIceConnectionState::Failed {
                                    set_status.set("Não foi possível conectar. Tente recarregar a página.".to_string());
                                }
                            })
                        };
                        pc.set_oniceconnectionstatechange(Some(oniceconnectionstatechange.as_ref().unchecked_ref()));
                        oniceconnectionstatechange.forget();

                        if let Ok(answer_sdp) = create_answer(&pc, &sdp).await {
                            if let Some(ws) = ws_slot.borrow().as_ref() {
                                ws.send(&ClientMessage::Answer { to: from.clone(), sdp: answer_sdp });
                            }
                        }

                        *pc_slot.borrow_mut() = Some(pc);
                        set_status.set("Conectado.".to_string());
                    });
                }
                ServerMessage::IceCandidate { candidate, sdp_mid, sdp_m_line_index, .. } => {
                    if let Some(pc) = pc_slot.borrow().as_ref() {
                        add_ice_candidate(pc, &candidate, sdp_mid, sdp_m_line_index);
                    }
                }
                ServerMessage::RoomClosed => {
                    set_status.set("O compartilhamento foi encerrado.".to_string());
                    if let Some(pc) = pc_slot.borrow_mut().take() {
                        pc.close();
                    }
                }
                _ => {}
            }
        }
    };

    match WsClient::connect("/ws", on_message) {
        Ok(ws) => {
            ws.on_open({
                let ws_slot = ws_slot.clone();
                let room_code = room_code.clone();
                move || {
                    if let Some(ws) = ws_slot.borrow().as_ref() {
                        ws.send(&ClientMessage::Join { room: room_code.clone() });
                    }
                }
            });
            ws.on_close(move || {
                set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
            });
            *ws_slot.borrow_mut() = Some(ws);
        }
        Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
    }
}
