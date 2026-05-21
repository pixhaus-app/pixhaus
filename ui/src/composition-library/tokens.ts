// `{token}` detection for saved PromptTemplates.
//
// Ported from `ai/src/compose/variables.rs::detect_tokens`. The logic is
// kept strictly isomorphic so the two implementations always agree on which
// keys a template declares.

/**
 * Returns the distinct `{token}` keys in `text`, in first-seen order.
 * `{{` and `}}` are literal brace escapes and yield no token.
 * Empty keys (i.e. `{}`) are skipped.
 */
export function detectTokens(text: string): string[] {
  const out: string[] = [];
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === "{") {
      // `{{` is a literal brace — skip both chars.
      if (text[i + 1] === "{") {
        i += 2;
        continue;
      }
      // Collect characters up to the matching `}`.
      let key = "";
      i += 1;
      while (i < text.length && text[i] !== "}") {
        key += text[i];
        i += 1;
      }
      // `closed` is true only when we stopped on a `}` (not end-of-string);
      // an unterminated `{key` is malformed and yields no token.
      const closed = i < text.length;
      i += 1; // advance past the closing `}` (or harmlessly past the end)
      if (closed && key.length > 0 && !out.includes(key)) {
        out.push(key);
      }
    } else if (c === "}") {
      // `}}` is a literal brace — skip both chars.
      if (text[i + 1] === "}") {
        i += 2;
      } else {
        i += 1;
      }
    } else {
      i += 1;
    }
  }
  return out;
}
