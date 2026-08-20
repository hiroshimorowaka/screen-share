use serde::{Deserialize, Serialize};

/// Identidade local do usuário (nick + cor). Vive fora de `client/` (que é
/// inteiramente bloqueado sob a feature `hydrate`) porque `home.rs`/`room.rs`
/// referenciam este tipo em assinaturas de função que precisam compilar tanto
/// sob `ssr` quanto sob `hydrate` — só as funções de leitura/escrita no
/// `localStorage` (em `client::storage`) são específicas de navegador.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub nick: String,
    pub color: String,
}

impl Default for Profile {
    fn default() -> Self {
        Self { nick: String::new(), color: crate::pages::palette::DEFAULT_COLOR.to_string() }
    }
}

/// Uma sala recentemente criada ou visitada, lembrada localmente (sem senha).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentRoom {
    pub code: String,
    pub name: String,
}
