use serde::{Deserialize, Serialize};

/// Fora de `client/` (bloqueado sob `hydrate`) porque `home.rs`/`room.rs`
/// referenciam este tipo em assinaturas que precisam compilar sob `ssr` também.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub nick: String,
    pub color: String,
}

impl Default for Profile {
    fn default() -> Self {
        Self { nick: String::new(), color: crate::ui::pages::palette::DEFAULT_COLOR.to_string() }
    }
}

// Deliberadamente sem campo de senha.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentRoom {
    pub code: String,
    pub name: String,
}
