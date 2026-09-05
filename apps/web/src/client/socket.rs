use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, WebSocket};

use screen_share_protocol::{ClientMessage, ServerMessage};

pub struct WsClient {
    socket: WebSocket,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    // Held so it's freed with the `WsClient`. `on_close` deliberately
    // stays a `once_into_js` leak instead: a socket always eventually
    // closes, so it fires (no leak), and its handler must outlive this
    // `WsClient` to run the reconnect check. `on_open` can genuinely never
    // fire — a failed `connect()` — so keeping its closure here means it's
    // dropped with the client rather than leaked.
    _on_open: RefCell<Option<Closure<dyn FnMut()>>>,
}

fn message_closure(
    on_message: impl Fn(ServerMessage) + 'static,
) -> Closure<dyn FnMut(MessageEvent)> {
    Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if let Some(text) = event.data().as_string() {
            if let Ok(msg) = serde_json::from_str::<ServerMessage>(&text) {
                on_message(msg);
            }
        }
    })
}

impl WsClient {
    pub fn connect(
        path: &str,
        on_message: impl Fn(ServerMessage) + 'static,
    ) -> Result<Self, JsValue> {
        let window = web_sys::window()
            .ok_or_else(|| JsValue::from_str("no window: not running in a browser"))?;
        let location = window.location();
        let protocol = if location.protocol()? == "https:" {
            "wss"
        } else {
            "ws"
        };
        let host = location.host()?;
        let url = format!("{protocol}://{host}{path}");

        let socket = WebSocket::new(&url)?;

        let on_message_cb = message_closure(on_message);
        socket.set_onmessage(Some(on_message_cb.as_ref().unchecked_ref()));

        Ok(Self {
            socket,
            _on_message: on_message_cb,
            _on_open: RefCell::new(None),
        })
    }

    pub fn set_on_message(&mut self, on_message: impl Fn(ServerMessage) + 'static) {
        let on_message_cb = message_closure(on_message);
        self.socket
            .set_onmessage(Some(on_message_cb.as_ref().unchecked_ref()));
        self._on_message = on_message_cb;
    }

    pub fn send(&self, msg: &ClientMessage) {
        if let Ok(json) = serde_json::to_string(msg) {
            let _ = self.socket.send_with_str(&json);
        }
    }

    pub fn on_open(&self, callback: impl FnOnce() + 'static) {
        let cb = Closure::once(callback);
        self.socket.set_onopen(Some(cb.as_ref().unchecked_ref()));
        *self._on_open.borrow_mut() = Some(cb);
    }

    pub fn on_close(&self, callback: impl FnOnce() + 'static) {
        let cb = Closure::once_into_js(callback);
        self.socket.set_onclose(Some(cb.as_ref().unchecked_ref()));
    }

    pub fn close(&self) {
        let _ = self.socket.close();
    }
}

impl Drop for WsClient {
    /// A dropped `WsClient` whose socket was never explicitly closed would
    /// otherwise leave the underlying browser `WebSocket` open until GC —
    /// long enough for the server's idle reap to fire. Closing on drop is
    /// idempotent with an earlier explicit `close()`.
    fn drop(&mut self) {
        let _ = self.socket.close();
    }
}
