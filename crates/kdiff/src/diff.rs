//! Structural diff over [`Node`] trees.
//!
//! Equality is decided by each node's content (merkle) hash, so identical
//! subtrees are skipped in O(1). Struct fields diff positionally (a kaitai type
//! always has the same ordered field set); lists diff via LCS on child hashes,
//! pairing adjacent delete/insert runs into recursive "modified" diffs.

use kdiff_abi::Node;
use std::fmt::Write;

pub struct DiffOptions {
    pub color: bool,
}

pub fn diff(old: &Node, new: &Node, opts: &DiffOptions) -> String {
    let mut r = Renderer { out: String::new(), color: opts.color };
    if old.content_hash() == new.content_hash() {
        return String::new();
    }
    let label = root_label(old);
    r.change(old, new, &label, 0);
    r.out
}

fn root_label(n: &Node) -> String {
    match n {
        Node::Struct { ty, .. } => ty.clone(),
        _ => "value".to_string(),
    }
}

struct Renderer {
    out: String,
    color: bool,
}

// ANSI colors, applied to a whole line.
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

impl Renderer {
    fn line(&mut self, sign: char, depth: usize, text: &str) {
        let indent = "  ".repeat(depth);
        let body = format!("{sign} {indent}{text}");
        if self.color {
            let color = match sign {
                '-' => RED,
                '+' => GREEN,
                '~' => CYAN,
                _ => DIM,
            };
            let _ = writeln!(self.out, "{color}{body}{RESET}");
        } else {
            let _ = writeln!(self.out, "{body}");
        }
    }

    /// Render the diff between two changed nodes under `label`.
    fn change(&mut self, old: &Node, new: &Node, label: &str, depth: usize) {
        match (old, new) {
            (Node::Struct { ty: a, fields: fa }, Node::Struct { ty: b, fields: fb }) if a == b => {
                self.line('~', depth, &format!("{label} ({a})"));
                for (fo, fne) in fa.iter().zip(fb.iter()) {
                    if fo.value.content_hash() != fne.value.content_hash() {
                        self.change(&fo.value, &fne.value, &fo.name, depth + 1);
                    }
                }
            }
            (Node::List { items: ia, .. }, Node::List { items: ib, .. }) => {
                self.line('~', depth, &format!("{label} [{} → {}]", ia.len(), ib.len()));
                self.list(ia, ib, depth + 1);
            }
            _ if same_scalar_kind(old, new) => {
                self.line('-', depth, &format!("{label}: {}", render_scalar(old)));
                self.line('+', depth, &format!("{label}: {}", render_scalar(new)));
            }
            // Different shapes entirely: show full removal then full addition.
            _ => {
                self.full(old, label, '-', depth);
                self.full(new, label, '+', depth);
            }
        }
    }

    /// Render a whole subtree, every line prefixed with `sign` (+/-).
    fn full(&mut self, node: &Node, label: &str, sign: char, depth: usize) {
        match node {
            Node::Struct { ty, fields } => {
                self.line(sign, depth, &format!("{label}: {ty}"));
                for f in fields {
                    self.full(&f.value, &f.name, sign, depth + 1);
                }
            }
            Node::List { items, .. } => {
                self.line(sign, depth, &format!("{label}: [{} items]", items.len()));
                for (i, it) in items.iter().enumerate() {
                    self.full(it, &format!("[{i}]"), sign, depth + 1);
                }
            }
            _ => self.line(sign, depth, &format!("{label}: {}", render_scalar(node))),
        }
    }

    fn list(&mut self, old: &[Node], new: &[Node], depth: usize) {
        let oh: Vec<u64> = old.iter().map(|n| n.content_hash()).collect();
        let nh: Vec<u64> = new.iter().map(|n| n.content_hash()).collect();
        let ops = lcs_ops(&oh, &nh);

        // Walk ops, batching runs of non-Equal edits so adjacent delete+insert
        // pairs render as a recursive "modified element" diff.
        let mut dels: Vec<usize> = Vec::new();
        let mut inss: Vec<usize> = Vec::new();
        let flush = |r: &mut Renderer, dels: &mut Vec<usize>, inss: &mut Vec<usize>| {
            let paired = dels.len().min(inss.len());
            for k in 0..paired {
                let oi = dels[k];
                let ni = inss[k];
                r.change(&old[oi], &new[ni], &format!("[{ni}]"), depth);
            }
            for &oi in &dels[paired..] {
                r.full(&old[oi], &format!("[{oi}]"), '-', depth);
            }
            for &ni in &inss[paired..] {
                r.full(&new[ni], &format!("[{ni}]"), '+', depth);
            }
            dels.clear();
            inss.clear();
        };

        for op in ops {
            match op {
                Op::Equal => flush(self, &mut dels, &mut inss),
                Op::Del(i) => dels.push(i),
                Op::Ins(j) => inss.push(j),
            }
        }
        flush(self, &mut dels, &mut inss);
    }
}

fn same_scalar_kind(a: &Node, b: &Node) -> bool {
    use Node::*;
    matches!(
        (a, b),
        (UInt(_), UInt(_))
            | (Int(_), Int(_))
            | (Float(_), Float(_))
            | (Str(_), Str(_))
            | (Bytes(_), Bytes(_))
            | (Enum { .. }, Enum { .. })
            | (Null, Null)
    )
}

fn render_scalar(n: &Node) -> String {
    match n {
        Node::UInt(v) => v.to_string(),
        Node::Int(v) => v.to_string(),
        Node::Float(v) => v.to_string(),
        Node::Str(s) => format!("{s:?}"),
        Node::Bytes(b) => render_bytes(b),
        Node::Enum { name, value, .. } => format!("{name}({value})"),
        Node::Null => "null".to_string(),
        Node::Struct { ty, .. } => format!("{ty} {{…}}"),
        Node::List { items, .. } => format!("[{} items]", items.len()),
    }
}

fn render_bytes(b: &[u8]) -> String {
    const MAX: usize = 16;
    let mut s = String::new();
    for (i, byte) in b.iter().take(MAX).enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{byte:02x}");
    }
    if b.len() > MAX {
        let _ = write!(s, " …({} bytes)", b.len());
    } else if b.is_empty() {
        s.push_str("(empty)");
    }
    s
}

// ---------------------------------------------------------------------------
// LCS edit script over hash sequences.
// ---------------------------------------------------------------------------

enum Op {
    Equal,
    Del(usize),
    Ins(usize),
}

fn lcs_ops(a: &[u64], b: &[u64]) -> Vec<Op> {
    let n = a.len();
    let m = b.len();
    // dp[i][j] = LCS length of a[i..] and b[j..].
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut ops = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            ops.push(Op::Equal);
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            ops.push(Op::Del(i));
            i += 1;
        } else {
            ops.push(Op::Ins(j));
            j += 1;
        }
    }
    while i < n {
        ops.push(Op::Del(i));
        i += 1;
    }
    while j < m {
        ops.push(Op::Ins(j));
        j += 1;
    }
    ops
}
