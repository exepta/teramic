//! Resolve command arguments against the identifiers in the command's own catalogue.
#[derive(Debug, PartialEq, Eq)]
pub enum LookupError {
    Unknown,
    Ambiguous,
}

impl LookupError {
    pub fn message(self, unknown: &'static str) -> &'static str {
        match self {
            Self::Unknown => unknown,
            Self::Ambiguous => "Ambiguous name. Use namespace::name. Tab lists the matching IDs.",
        }
    }
}

fn local(id: &str) -> &str {
    id.split_once("::").map_or(id, |(_, local)| local)
}

pub fn resolve<'a, T>(
    input: &str,
    entries: impl IntoIterator<Item = (&'a str, T)>,
) -> Result<T, LookupError> {
    let qualified = input.contains("::");
    let mut matches = entries
        .into_iter()
        .filter(|(id, _)| input.eq_ignore_ascii_case(if qualified { id } else { local(id) }));
    let (_, value) = matches.next().ok_or(LookupError::Unknown)?;
    if matches.next().is_some() {
        return Err(LookupError::Ambiguous);
    }
    Ok(value)
}

/// With a colliding short name, offer both qualified choices while it is being typed.
pub fn completions<'a>(input: &str, ids: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let ids: Vec<_> = ids.into_iter().collect();
    if input.contains(':') {
        return ids.into_iter().filter(|id| id.starts_with(input)).collect();
    }
    let mut counts = std::collections::HashMap::new();
    for &id in &ids {
        *counts.entry(local(id)).or_insert(0usize) += 1;
    }
    ids.into_iter()
        .filter(|id| local(id).starts_with(input))
        .map(|id| {
            let name = local(id);
            if counts[name] > 1 { id } else { name }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_local_names_require_a_namespace() {
        let entries = [
            ("teramic::desert", 1),
            ("mod_name::desert", 2),
            ("mod_name::fen", 3),
        ];
        assert_eq!(resolve("desert", entries), Err(LookupError::Ambiguous));
        assert_eq!(resolve("teramic::desert", entries), Ok(1));
        assert_eq!(resolve("mod_name::desert", entries), Ok(2));
        assert_eq!(
            resolve("another::desert", entries),
            Err(LookupError::Unknown)
        );
        assert_eq!(resolve("fen", entries), Ok(3));
        assert_eq!(
            completions("des", entries.map(|(id, _)| id)),
            ["teramic::desert", "mod_name::desert"]
        );
    }
}
