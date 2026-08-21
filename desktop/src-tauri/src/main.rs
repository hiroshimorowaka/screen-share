use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

fn main() {
    // WebKitGTK's DMA-BUF video renderer fails to initialize on several
    // Linux driver/compositor combinations and falls back to rendering
    // solid black instead of erroring — this forces the older, reliable
    // rendering path. See tauri-apps/tauri#9394.
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    // Belt-and-suspenders alongside the DMA-BUF workaround above: forces
    // WebKitGTK's plain (non-GPU-accelerated) compositor, which several
    // reports found necessary to get WebRTC video actually rendering
    // (rather than blank/black) on Linux.
    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");

    tauri::Builder::default()
        .setup(|app| {
            let show = MenuItemBuilder::with_id("show", "Abrir").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Sair").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.with_webview(|webview| {
                    use webkit2gtk::glib::Cast;
                    use webkit2gtk::{PermissionRequestExt, WebViewExt};

                    let webview = webview.inner();
                    webview.connect_permission_request(|_webview, request| {
                        if let Some(media_request) =
                            request.downcast_ref::<webkit2gtk::UserMediaPermissionRequest>()
                        {
                            media_request.allow();
                            return true;
                        }
                        false
                    });
                });
            }

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
