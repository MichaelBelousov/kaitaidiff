//! Work around a kaitai-struct-compiler (rust target, 0.11) bug: field ids that
//! are Rust keywords (`type`, `match`, `ref`, ...) are emitted unescaped, e.g.
//! `pub fn type(&self)` / `self.type`, which is not valid Rust.
//!
//! [`esc_ident`] escapes a single identifier for our generated getter calls.
//! [`escape_rust_keywords`] patches the kaitai-generated source, escaping only
//! identifiers in the three contexts where a *field* (not a legitimate keyword
//! such as the `type Root = ...` associated-type line) can appear:
//!   1. after `.`            (field access / method call: `self.type`, `o.type()`)
//!   2. after `fn `          (getter definition: `pub fn type(`)
//!   3. as a `field:` decl   (struct field / struct-literal key, line-leading)

/// Keywords that can be raw identifiers. `self`, `Self`, `super`, `crate` are
/// excluded because `r#self` etc. are themselves illegal — but kaitai already
/// renames those (`_self`, ...), so they never collide in practice.
const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "dyn", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "static", "struct", "trait", "true", "type", "unsafe", "use", "where", "while", "async",
    "await", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof",
    "unsized", "virtual", "yield", "try",
];

fn is_keyword(w: &str) -> bool {
    KEYWORDS.contains(&w)
}

/// Escape a field identifier for use in generated Rust source.
pub fn esc_ident(id: &str) -> String {
    if is_keyword(id) {
        format!("r#{id}")
    } else {
        id.to_string()
    }
}

fn is_ident_start(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphabetic()
}
fn is_ident_continue(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphanumeric()
}

pub fn escape_rust_keywords(src: &str) -> String {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(n + 16);
    let mut i = 0;
    let mut line_has_nonws = false;

    while i < n {
        let c = b[i];
        if c == b'\n' {
            line_has_nonws = false;
            out.push('\n');
            i += 1;
            continue;
        }
        if is_ident_start(c) {
            let start = i;
            let mut j = i;
            while j < n && is_ident_continue(b[j]) {
                j += 1;
            }
            let word = &src[start..j];

            // Don't re-escape an already-raw identifier `r#word`.
            let already_raw = start >= 2 && &src[start - 2..start] == "r#";

            let escape = !already_raw
                && is_keyword(word)
                && {
                    let after_dot = start > 0 && b[start - 1] == b'.';
                    let after_fn = start >= 3 && &src[start - 3..start] == "fn ";
                    let field_decl = !line_has_nonws
                        && j < n
                        && b[j] == b':'
                        && (j + 1 >= n || b[j + 1] != b':');
                    after_dot || after_fn || field_decl
                };

            if escape {
                out.push_str("r#");
            }
            out.push_str(word);
            line_has_nonws = true;
            i = j;
            continue;
        }

        if c != b' ' && c != b'\t' {
            line_has_nonws = true;
        }
        out.push(c as char);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_field_contexts_only() {
        let src = "    type: RefCell<String>,\n    pub fn type(&self) -> Ref<String> {\n        self.type.borrow()\n    }\n    type Root = Png;\n";
        let out = escape_rust_keywords(src);
        assert!(out.contains("r#type: RefCell"), "{out}");
        assert!(out.contains("fn r#type("), "{out}");
        assert!(out.contains("self.r#type.borrow"), "{out}");
        // associated-type line must be untouched
        assert!(out.contains("type Root = Png;"), "{out}");
    }

    #[test]
    fn leaves_legit_keywords() {
        let src = "match flag {\n    0 => x as u8,\n}\n";
        assert_eq!(escape_rust_keywords(src), src);
    }
}
