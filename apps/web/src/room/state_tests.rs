//! `RoomState::new` wires a fresh, independent signal into every field.

use super::*;

#[test]
fn new_wires_distinct_writable_signals() {
    let owner = Owner::new();
    owner.with(|| {
        let state = RoomState::new();

        // Defaults match the pre-consolidation `RoomPage` initial values.
        assert_eq!(state.status.get_untracked(), "Informe o nick da sala.");
        assert!(!state.authenticated.get_untracked());
        assert!(state.requires_password.get_untracked());
        assert!(state.members.get_untracked().is_empty());
        assert!(state.my_peer_id.get_untracked().is_none());
        assert!(!state.is_sharing.get_untracked());
        assert!(state.controls_visible.get_untracked());
        assert!(!state.hide_idle.get_untracked());

        // Writing one field leaves an unrelated one untouched — the
        // fields are not aliasing the same handle.
        state.set_authenticated.set(true);
        assert!(state.authenticated.get_untracked());
        assert!(!state.is_sharing.get_untracked());

        state.watching.update(|w| {
            w.insert("peer-1".to_string());
        });
        assert!(state.watching.get_untracked().contains("peer-1"));
        assert!(state.connection_errors.get_untracked().is_empty());
    });
}
