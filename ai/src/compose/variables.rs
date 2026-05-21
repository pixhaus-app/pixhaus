//! `{token}` parsing and substitution for saved Prompts.

use std::collections::BTreeMap;

use thiserror::Error;

/// Error returned while substituting prompt variables.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VarError {
    /// A required `{token}` had no value from any source.
    #[error("unfilled variable `{0}`")]
    Unfilled(String),
    /// A `{` opened a placeholder that never closed.
    #[error("malformed placeholder near byte {0}")]
    Malformed(usize),
}

/// A token resolver: returns the value for a key, or `None` to fall through.
pub trait VarSource {
    /// Returns the value bound to `key`, if this source defines one.
    fn get(&self, key: &str) -> Option<String>;
}

impl VarSource for BTreeMap<String, String> {
    fn get(&self, key: &str) -> Option<String> {
        BTreeMap::get(self, key).cloned()
    }
}

/// Returns the distinct `{token}` keys appearing in `text`, in first-seen
/// order. `{{`/`}}` are literal braces and yield no tokens.
#[must_use]
pub fn detect_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        match c {
            '{' => {
                if matches!(chars.peek(), Some((_, '{'))) {
                    chars.next();
                    continue;
                }
                let mut key = String::new();
                for (_, k) in chars.by_ref() {
                    if k == '}' {
                        break;
                    }
                    key.push(k);
                }
                if !key.is_empty() && !out.contains(&key) {
                    out.push(key);
                }
            }
            '}' => {
                if matches!(chars.peek(), Some((_, '}'))) {
                    chars.next();
                }
            }
            _ => {}
        }
    }
    out
}

/// Substitutes every `{token}` in `text` using `sources` in order (first hit
/// wins). `{{`/`}}` collapse to literal braces. Errors on the first token no
/// source can fill.
pub fn substitute(text: &str, sources: &[&dyn VarSource]) -> Result<String, VarError> {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '{' if bytes.get(i + 1) == Some(&b'{') => {
                out.push('{');
                i += 2;
            }
            '}' if bytes.get(i + 1) == Some(&b'}') => {
                out.push('}');
                i += 2;
            }
            '{' => {
                let start = i + 1;
                let end = text[start..]
                    .find('}')
                    .map(|o| start + o)
                    .ok_or(VarError::Malformed(i))?;
                let key = &text[start..end];
                let val = sources
                    .iter()
                    .find_map(|s| s.get(key))
                    .ok_or_else(|| VarError::Unfilled(key.to_string()))?;
                out.push_str(&val);
                i = end + 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn detects_tokens_in_order_without_duplicates() {
        assert_eq!(
            detect_tokens("a {species} {x} {species}"),
            vec!["species", "x"]
        );
    }

    #[test]
    fn detects_no_tokens_in_escaped_braces() {
        assert!(detect_tokens("{{not a token}}").is_empty());
    }

    #[test]
    fn substitutes_from_first_source() {
        let primary = map(&[("species", "orc")]);
        let fallback = map(&[("species", "human")]);
        let out = substitute("a {species}", &[&primary, &fallback]).unwrap();
        assert_eq!(out, "a orc");
    }

    #[test]
    fn falls_through_to_second_source() {
        let primary = map(&[]);
        let fallback = map(&[("species", "human")]);
        assert_eq!(
            substitute("a {species}", &[&primary, &fallback]).unwrap(),
            "a human"
        );
    }

    #[test]
    fn errors_on_unfilled() {
        let empty = map(&[]);
        assert_eq!(
            substitute("a {x}", &[&empty]),
            Err(VarError::Unfilled("x".into()))
        );
    }

    #[test]
    fn keeps_literal_braces() {
        let empty = map(&[]);
        assert_eq!(substitute("{{x}}", &[&empty]).unwrap(), "{x}");
    }

    #[test]
    fn errors_on_unterminated() {
        let empty = map(&[]);
        assert!(matches!(
            substitute("a {x", &[&empty]),
            Err(VarError::Malformed(_))
        ));
    }

    proptest! {
        #[test]
        fn plain_text_without_braces_is_unchanged(s in "[a-zA-Z0-9 ,.]{0,64}") {
            let empty = map(&[]);
            prop_assert_eq!(substitute(&s, &[&empty]).unwrap(), s);
        }
    }
}
