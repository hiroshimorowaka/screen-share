use leptos::prelude::*;

/// Accepts either the bare room code or the full invite link. Normalizes to
/// uppercase — `generate_room_code` only ever generates uppercase codes, so
/// pasting a lowercase link would otherwise never match the real room.
///
/// `cfg(any(test, feature = "hydrate"))`, not just `hydrate`: avoids the
/// dead-code warning on an `ssr`-only build, while keeping the function
/// plain Rust (no `web-sys`) and testable without a browser.
#[cfg(any(test, feature = "hydrate"))]
fn extract_room_code(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let after_marker = match trimmed.find("/r/") {
        Some(idx) => &trimmed[idx + "/r/".len()..],
        None => trimmed,
    };
    let code = after_marker.split(['/', '?', '#']).next().unwrap_or("").trim();
    if code.is_empty() {
        None
    } else {
        Some(code.to_ascii_uppercase())
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn join_room_handler(_join_input: ReadSignal<String>, _set_join_status: WriteSignal<String>) -> impl Fn(leptos::ev::SubmitEvent) + 'static {
    move |ev: leptos::ev::SubmitEvent| ev.prevent_default()
}

/// Only resolves the code and navigates to `/r/{code}` — nick, color, and
/// password are left for the room page's own entry gate.
#[cfg(feature = "hydrate")]
pub fn join_room_handler(join_input: ReadSignal<String>, set_join_status: WriteSignal<String>) -> impl Fn(leptos::ev::SubmitEvent) + 'static {
    use leptos_router::hooks::use_navigate;

    move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let Some(code) = extract_room_code(&join_input.get_untracked()) else {
            set_join_status.set("Informe o código da sala ou o link completo do convite.".to_string());
            return;
        };

        let navigate = use_navigate();
        navigate(&format!("/r/{code}"), Default::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_room_code_accepts_a_bare_code() {
        assert_eq!(extract_room_code("ab3d9f2k"), Some("AB3D9F2K".to_string()));
    }

    #[test]
    fn extract_room_code_accepts_a_full_link() {
        assert_eq!(extract_room_code("https://example.com/r/AB3D9F2K"), Some("AB3D9F2K".to_string()));
    }

    #[test]
    fn extract_room_code_strips_trailing_slash_and_query_string() {
        assert_eq!(extract_room_code("https://example.com/r/AB3D9F2K/?x=1"), Some("AB3D9F2K".to_string()));
    }

    #[test]
    fn extract_room_code_trims_surrounding_whitespace() {
        assert_eq!(extract_room_code("  AB3D9F2K  "), Some("AB3D9F2K".to_string()));
    }

    #[test]
    fn extract_room_code_rejects_blank_input() {
        assert_eq!(extract_room_code("   "), None);
    }
}
