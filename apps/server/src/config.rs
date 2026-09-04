//! All server runtime configuration, read from the environment at
//! process start (see CLAUDE.md §Configuration — the deployed artifact
//! carries no config file).

use leptos::prelude::*;
use screen_share_signaling::handshake::HandshakeConfig;
use screen_share_signaling::turn::TurnConfig;

/// The environment-derived configuration for one server process.
pub struct ServerConfig {
    /// Leptos site options (bind address, output name, asset paths).
    pub leptos_options: LeptosOptions,
    /// `true` outside production (`cargo leptos watch`), where the CSP is
    /// loosened just enough for the live-reload WebSocket.
    pub dev_csp: bool,
    /// `None` when no TURN server is configured — clients then get
    /// STUN-only ICE.
    pub turn: Option<TurnConfig>,
    /// Per-deployment handshake parameters (trusted-proxy client-IP
    /// resolution, the SSR rate-limit identity key).
    pub handshake: HandshakeConfig,
}

impl ServerConfig {
    /// Reads every knob this process needs from the environment.
    ///
    /// # Errors
    /// Fails if `get_configuration` can't read the Leptos env vars, or if
    /// `TURN_SECRET` is set but malformed — the process aborts rather
    /// than run a relay with a weak secret (finding F13).
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let conf = get_configuration(None)?;
        let leptos_options = conf.leptos_options;
        // Non-PROD => `cargo leptos watch`: its live-reload WebSocket
        // needs a slightly looser CSP (see `middleware::security`).
        let dev_csp = !matches!(leptos_options.env, leptos::config::Env::PROD);
        let turn = TurnConfig::from_env()?;
        let handshake = HandshakeConfig::from_env();
        Ok(Self {
            leptos_options,
            dev_csp,
            turn,
            handshake,
        })
    }
}
