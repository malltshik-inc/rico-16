//! SPEC §16.2 — Rhai syntax highlighting.

/// The class of a highlighted span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    /// A Rhai keyword.
    Keyword,
    /// A literal (`true`, `false`, `()`).
    Literal,
    /// A string literal.
    String,
    /// A comment.
    Comment,
    /// A numeric literal.
    Number,
    /// An identifier.
    Ident,
    /// Anything else.
    Punct,
}

/// SPEC §9.1, §11.1 — the reserved words the editor highlights.
pub const KEYWORDS: [&str; 28] = [
    "let", "const", "fn", "private", "if", "else", "switch", "while", "do", "until", "loop", "for",
    "in", "break", "continue", "return", "import", "export", "as", "try", "catch", "throw", "this",
    "static", "shared", "global", "true", "false",
];

/// Split one line into highlighted spans.
pub fn highlight(line: &str) -> Vec<(Token, &str)> {
    let bytes = line.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let start = i;
        let c = bytes[i];

        // Comments run to the end of the line.
        if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            spans.push((Token::Comment, &line[start..]));
            break;
        }

        // String literals: `"`, `'` and the interpolated backtick.
        if c == b'"' || c == b'\'' || c == b'`' {
            let quote = c;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            spans.push((Token::String, &line[start..i.min(bytes.len())]));
            continue;
        }

        // Numbers.
        if c.is_ascii_digit() {
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_')
            {
                i += 1;
            }
            spans.push((Token::Number, &line[start..i]));
            continue;
        }

        // Identifiers and keywords.
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &line[start..i];
            let token = if word == "true" || word == "false" {
                Token::Literal
            } else if KEYWORDS.contains(&word) {
                Token::Keyword
            } else {
                Token::Ident
            };
            spans.push((token, word));
            continue;
        }

        // Punctuation and whitespace, one char at a time.
        let width = utf8_len(c);
        i = (i + width).min(bytes.len());
        spans.push((Token::Punct, &line[start..i]));
    }

    spans
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else {
        4
    }
}
