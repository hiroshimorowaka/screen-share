/// (protocol id, border/avatar color, card background color).
const PALETTE: &[(&str, &str, &str)] = &[
    ("coral", "#ff6b6b", "#3a1f1f"),
    ("amber", "#ffb347", "#3a2a14"),
    ("gold", "#ffd93d", "#3a3316"),
    ("lime", "#a8e063", "#263a1a"),
    ("teal", "#4ecdc4", "#17322f"),
    ("sky", "#45b7ff", "#16283a"),
    ("periwinkle", "#7c83fd", "#23233f"),
    ("violet", "#b57cff", "#2c2140"),
    ("pink", "#ff6fb5", "#3a1f30"),
    ("slate", "#b0b8c1", "#2a2d31"),
];

pub const DEFAULT_COLOR: &str = "coral";

pub fn palette_ids() -> impl Iterator<Item = &'static str> {
    PALETTE.iter().map(|(id, _, _)| *id)
}

pub fn color_hex(id: &str) -> (&'static str, &'static str) {
    PALETTE
        .iter()
        .find(|(entry_id, _, _)| *entry_id == id)
        .map(|(_, border, bg)| (*border, *bg))
        .unwrap_or(("#b0b8c1", "#2a2d31"))
}

pub fn avatar_letter(nick: &str) -> String {
    nick.trim()
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
