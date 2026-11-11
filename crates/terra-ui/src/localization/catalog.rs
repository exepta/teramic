use fluent_bundle::FluentArgs;
use serde::Deserialize;
use std::collections::BTreeMap;
use tilt_ui::UiLocalization;

#[derive(Default, Deserialize)]
pub(super) struct Catalog {
    pub exact: BTreeMap<String, String>,
    pub messages: BTreeMap<String, String>,
    pub patterns: Vec<Pattern>,
}

#[derive(Deserialize)]
pub(super) struct Pattern {
    pub id: String,
    pub literals: Vec<String>,
    pub arguments: Vec<String>,
    #[serde(default)]
    pub numeric: Vec<bool>,
    #[serde(default)]
    pub trimmed: bool,
}

impl Catalog {
    pub fn embedded() -> Result<Self, serde_json::Error> {
        serde_json::from_str(include_str!("../../../../src-ui/locales/index.json"))
    }

    pub fn text(&self, localization: &UiLocalization, source: &str) -> String {
        if let Some(key) = self.exact.get(source) {
            return super::translate(localization, key, source);
        }
        // Empty/numeric values are very common in HUDs; never search message patterns for them.
        if source.len() > 8192 || !source.chars().any(char::is_alphabetic) {
            return source.to_owned();
        }
        for pattern in &self.patterns {
            if let Some(captures) = pattern.capture(source) {
                let mut arguments = FluentArgs::new();
                for (name, value) in pattern.arguments.iter().zip(captures) {
                    // Captured names and arbitrary server details are kept verbatim.
                    // These values have already been formatted by the producer. In particular,
                    // parsing u64 world seeds through f64 would silently round their digits.
                    arguments.set(name, value);
                }
                if let Some(text) = localization.translate(&pattern.id, Some(&arguments)) {
                    return if pattern.trimmed {
                        text.trim_matches('\n').to_owned()
                    } else {
                        text
                    };
                }
            }
        }
        // Individual item tooltips and server replies can contain several independent messages.
        if source.contains('\n') {
            return source
                .split('\n')
                .map(|line| self.text(localization, line))
                .collect::<Vec<_>>()
                .join("\n");
        }
        source.to_owned()
    }
}

impl Pattern {
    fn capture<'a>(&self, source: &'a str) -> Option<Vec<&'a str>> {
        if self.arguments.is_empty() || self.literals.len() != self.arguments.len() + 1 {
            return None;
        }
        let mut remaining = source.strip_prefix(self.literals.first()?.as_str())?;
        let mut values = Vec::with_capacity(self.arguments.len());
        for (index, literal) in self.literals.iter().skip(1).enumerate() {
            if index + 1 == self.arguments.len() {
                values.push(remaining.strip_suffix(literal)?);
                remaining = "";
            } else {
                // Adjacent arguments cannot be reconstructed reliably from formatted text.
                if literal.is_empty() {
                    return None;
                }
                let end = remaining.find(literal)?;
                values.push(&remaining[..end]);
                remaining = &remaining[end + literal.len()..];
            }
        }
        if !self.literals.iter().any(|literal| literal.contains('\n'))
            && values.iter().any(|value| value.contains('\n'))
        {
            return None;
        }
        if self
            .numeric
            .iter()
            .zip(&values)
            .any(|(numeric, value)| *numeric && value.parse::<f64>().is_err())
        {
            return None;
        }
        remaining.is_empty().then_some(values)
    }
}
