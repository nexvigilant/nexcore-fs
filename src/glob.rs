//! Shell glob pattern matching (replaces `glob` crate).
//!
//! Zero external dependencies. Implements basic glob matching: `*`, `?`, `[abc]`, `[a-z]`.
//! Covers the NexCore API surface: `Pattern::new()` and `Pattern::matches()`.

use std::fmt;
use std::path::Path;

/// A compiled glob pattern.
///
/// # Examples
/// ```
/// use nexcore_fs::glob::Pattern;
///
/// let pat = Pattern::new("*.rs").unwrap();
/// assert!(pat.matches("lib.rs"));
/// assert!(!pat.matches("lib.txt"));
/// ```
#[derive(Debug, Clone)]
pub struct Pattern {
    source: String,
    tokens: Vec<Token>,
}

#[derive(Debug, Clone)]
enum Token {
    /// Match exactly this character
    Char(char),
    /// Match any single character
    Any,
    /// Match zero or more characters (no path separators)
    Star,
    /// Match any character in the class
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

/// Error returned when a glob pattern is invalid.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct PatternError {
    pub pos: usize,
    pub msg: String,
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid glob at position {}: {}", self.pos, self.msg)
    }
}

impl std::error::Error for PatternError {}

impl Pattern {
    /// Compile a glob pattern string.
    ///
    /// Supported syntax:
    /// - `*` matches any sequence of non-separator characters
    /// - `?` matches any single non-separator character
    /// - `[abc]` matches any character in the set
    /// - `[a-z]` matches any character in the range
    /// - `[!abc]` or `[^abc]` matches any character NOT in the set
    #[allow(
        clippy::indexing_slicing,
        reason = "all indices are guarded by explicit `i < chars.len()` and `i + N < chars.len()` checks before use"
    )]
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "loop counter `i` is bounded by `chars.len()` at every increment site; increments of 1-3 cannot overflow usize on any supported platform"
    )]
    pub fn new(pattern: &str) -> Result<Self, PatternError> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0usize;

        while i < chars.len() {
            match chars[i] {
                '*' => {
                    // Collapse consecutive stars
                    while i + 1 < chars.len() && chars[i + 1] == '*' {
                        i += 1;
                    }
                    tokens.push(Token::Star);
                }
                '?' => tokens.push(Token::Any),
                '[' => {
                    i += 1;
                    if i >= chars.len() {
                        return Err(PatternError {
                            pos: i - 1,
                            msg: "unclosed character class".to_string(),
                        });
                    }
                    let negated = chars[i] == '!' || chars[i] == '^';
                    if negated {
                        i += 1;
                    }
                    let mut ranges = Vec::new();
                    while i < chars.len() && chars[i] != ']' {
                        let start = chars[i];
                        if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i + 2] != ']' {
                            let end = chars[i + 2];
                            ranges.push((start, end));
                            i += 3;
                        } else {
                            ranges.push((start, start));
                            i += 1;
                        }
                    }
                    if i >= chars.len() {
                        return Err(PatternError {
                            pos: i,
                            msg: "unclosed character class".to_string(),
                        });
                    }
                    tokens.push(Token::Class { negated, ranges });
                }
                '\\' => {
                    // Escape next character
                    i += 1;
                    if i < chars.len() {
                        tokens.push(Token::Char(chars[i]));
                    }
                }
                c => tokens.push(Token::Char(c)),
            }
            i += 1;
        }

        Ok(Self {
            source: pattern.to_string(),
            tokens,
        })
    }

    /// Test whether the given string matches this pattern.
    pub fn matches(&self, text: &str) -> bool {
        self.matches_from(&self.tokens, text.chars().collect::<Vec<_>>().as_slice())
    }

    /// Test whether a path matches this pattern (uses platform path separator).
    pub fn matches_path(&self, path: &Path) -> bool {
        self.matches(&path.to_string_lossy())
    }

    /// Returns the original pattern string.
    pub fn as_str(&self) -> &str {
        &self.source
    }

    #[allow(
        clippy::indexing_slicing,
        reason = "all indices are guarded by explicit `si < text.len()`, `ti < tokens.len()`, and `si > 0` checks immediately before use"
    )]
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "counters `ti`, `si`, and `star_si` are bounded by `tokens.len()` and `text.len()` respectively; `st + 1` is safe because `st` was stored from a valid token index"
    )]
    fn matches_from(&self, tokens: &[Token], text: &[char]) -> bool {
        let mut ti = 0usize; // token index
        let mut si = 0usize; // string index
        let mut star_ti: Option<usize> = None; // last star token position
        let mut star_si = 0usize; // string position when star was seen

        while si < text.len() {
            if ti < tokens.len() {
                match &tokens[ti] {
                    Token::Char(c) => {
                        if text[si] == *c {
                            ti += 1;
                            si += 1;
                            continue;
                        }
                    }
                    Token::Any => {
                        if text[si] != '/' && text[si] != '\\' {
                            ti += 1;
                            si += 1;
                            continue;
                        }
                    }
                    Token::Star => {
                        star_ti = Some(ti);
                        star_si = si;
                        ti += 1;
                        continue;
                    }
                    Token::Class { negated, ranges } => {
                        let c = text[si];
                        let in_class = ranges.iter().any(|&(lo, hi)| c >= lo && c <= hi);
                        if in_class != *negated {
                            ti += 1;
                            si += 1;
                            continue;
                        }
                    }
                }
            }
            // Mismatch — backtrack to last star
            if let Some(st) = star_ti {
                ti = st + 1;
                star_si += 1;
                si = star_si;
                // Star doesn't match path separators
                if si > 0 && (text[si - 1] == '/' || text[si - 1] == '\\') {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Skip trailing stars
        while ti < tokens.len() {
            if matches!(tokens[ti], Token::Star) {
                ti += 1;
            } else {
                break;
            }
        }
        ti == tokens.len()
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_matches_any_chars() {
        let p = Pattern::new("*.rs").ok();
        assert!(p.is_some());
        let p = p.unwrap();
        assert!(p.matches("lib.rs"));
        assert!(p.matches("main.rs"));
        assert!(!p.matches("lib.txt"));
        assert!(!p.matches("src/lib.rs")); // star doesn't cross /
    }

    #[test]
    fn question_mark_matches_single() {
        let p = Pattern::new("file?.txt").unwrap();
        assert!(p.matches("file1.txt"));
        assert!(p.matches("fileA.txt"));
        assert!(!p.matches("file12.txt"));
        assert!(!p.matches("file.txt"));
    }

    #[test]
    fn character_class() {
        let p = Pattern::new("[abc].txt").unwrap();
        assert!(p.matches("a.txt"));
        assert!(p.matches("b.txt"));
        assert!(!p.matches("d.txt"));
    }

    #[test]
    fn character_range() {
        let p = Pattern::new("[a-z].txt").unwrap();
        assert!(p.matches("m.txt"));
        assert!(!p.matches("M.txt"));
        assert!(!p.matches("1.txt"));
    }

    #[test]
    fn negated_class() {
        let p = Pattern::new("[!0-9].txt").unwrap();
        assert!(p.matches("a.txt"));
        assert!(!p.matches("5.txt"));
    }

    #[test]
    fn exact_match() {
        let p = Pattern::new("hello.rs").unwrap();
        assert!(p.matches("hello.rs"));
        assert!(!p.matches("hello.txt"));
    }

    #[test]
    fn empty_pattern() {
        let p = Pattern::new("").unwrap();
        assert!(p.matches(""));
        assert!(!p.matches("a"));
    }

    #[test]
    fn star_at_start() {
        let p = Pattern::new("*test*").unwrap();
        assert!(p.matches("test"));
        assert!(p.matches("mytest.rs"));
        assert!(p.matches("testing"));
    }

    #[test]
    fn unclosed_bracket_errors() {
        assert!(Pattern::new("[abc").is_err());
    }

    #[test]
    fn matches_path() {
        let p = Pattern::new("*.rs").unwrap();
        assert!(p.matches_path(Path::new("lib.rs")));
        assert!(!p.matches_path(Path::new("lib.txt")));
    }

    #[test]
    fn escaped_special_chars() {
        let p = Pattern::new("file\\*.txt").unwrap();
        assert!(p.matches("file*.txt"));
        assert!(!p.matches("file1.txt"));
    }
}
