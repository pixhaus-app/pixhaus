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
                let mut closed = false;
                for (_, k) in chars.by_ref() {
                    if k == '}' {
                        closed = true;
                        break;
                    }
                    key.push(k);
                }
                // Only a properly closed `{key}` yields a token; an unterminated
                // `{key` is malformed and contributes nothing.
                if closed && !key.is_empty() && !out.contains(&key) {
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
    // Iterate Unicode scalars (not bytes) so multi-byte characters in the
    // literal passthrough survive intact. `{`/`}` are ASCII, so the byte
    // offsets used for key slicing always land on char boundaries.
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '{' if matches!(chars.peek(), Some((_, '{'))) => {
                chars.next();
                out.push('{');
            }
            '}' if matches!(chars.peek(), Some((_, '}'))) => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let start = i + 1;
                let end = text[start..].find('}').map(|o| start + o).ok_or(VarError::Malformed(i))?;
                let key = &text[start..end];
                let val = sources.iter().find_map(|s| s.get(key)).ok_or_else(|| VarError::Unfilled(key.to_string()))?;
                out.push_str(&val);
                // Advance past the key and its closing `}` (at byte `end`).
                while let Some(&(pos, _)) = chars.peek() {
                    if pos > end {
                        break;
                    }
                    chars.next();
                }
            }
            _ => out.push(c),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn detects_tokens_in_order_without_duplicates() {
        assert_eq!(detect_tokens("a {species} {x} {species}"), vec!["species", "x"]);
    }

    #[test]
    fn detects_no_tokens_in_escaped_braces() {
        assert!(detect_tokens("{{not a token}}").is_empty());
    }

    #[test]
    fn detects_no_token_for_unterminated_brace() {
        assert!(detect_tokens("a {name").is_empty());
        assert_eq!(detect_tokens("{a} {b"), vec!["a"]);
    }

    #[test]
    fn substitution_preserves_non_ascii() {
        let vars = map(&[("x", "orc")]);
        assert_eq!(substitute("héllo {x} 世界 🎨", &[&vars]).unwrap(), "héllo orc 世界 🎨");
    }

    #[test]
    fn substitution_with_non_ascii_value_and_key_after() {
        let vars = map(&[("名前", "勇者")]);
        assert_eq!(substitute("名は {名前} です", &[&vars]).unwrap(), "名は 勇者 です");
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
        assert_eq!(substitute("a {species}", &[&primary, &fallback]).unwrap(), "a human");
    }

    #[test]
    fn errors_on_unfilled() {
        let empty = map(&[]);
        assert_eq!(substitute("a {x}", &[&empty]), Err(VarError::Unfilled("x".into())));
    }

    #[test]
    fn keeps_literal_braces() {
        let empty = map(&[]);
        assert_eq!(substitute("{{x}}", &[&empty]).unwrap(), "{x}");
    }

    #[test]
    fn errors_on_unterminated() {
        let empty = map(&[]);
        assert!(matches!(substitute("a {x", &[&empty]), Err(VarError::Malformed(_))));
    }

    proptest! {
        #[test]
        fn plain_text_without_braces_is_unchanged(s in "[a-zA-Z0-9 ,.]{0,64}") {
            let empty = map(&[]);
            prop_assert_eq!(substitute(&s, &[&empty]).unwrap(), s);
        }
    }
}
