use serde::{Deserialize, Serialize};

pub const CHAT_TEXT_LIMIT: usize = 256;
pub const CHAT_HISTORY_LIMIT: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocateResult {
    pub name: String,
    /// Feet position; the server independently resolves the character center.
    pub position: [i64; 3],
    /// A per-player server-issued capability, never arbitrary client coordinates.
    pub token: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatEntry {
    pub id: u64,
    pub author: Option<String>,
    pub text: String,
    pub locate: Option<LocateResult>,
}

pub(crate) fn sanitize(text: &str) -> Option<String> {
    if text.len() > CHAT_TEXT_LIMIT * 4 || text.chars().count() > CHAT_TEXT_LIMIT {
        return None;
    }
    let value: String = text
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .collect();
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_text_is_bounded_and_control_characters_cannot_spoof_chat_rows() {
        assert_eq!(
            sanitize("  hello\nworld\u{202e} "),
            Some("helloworld".into())
        );
        assert_eq!(sanitize("\n\t"), None);
        assert_eq!(sanitize(&"x".repeat(257)), None);
        assert!(sanitize(&"ä".repeat(256)).is_some());
    }
}
