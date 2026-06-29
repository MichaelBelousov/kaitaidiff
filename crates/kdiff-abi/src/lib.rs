//! Shared contract between the `kdiff` CLI and the compiled per-schema plugins.
//!
//! A plugin parses a byte buffer with a kaitai-generated parser and produces a
//! generic [`Node`] tree. The tree is serialized to JSON and handed back across
//! a C ABI boundary; the CLI deserializes it and runs the structural diff.

use serde::{Deserialize, Serialize};

/// Bumped whenever the plugin <-> CLI contract changes. The CLI refuses to load
/// a plugin whose `kdiff_abi_version()` does not match.
pub const ABI_VERSION: u32 = 1;

/// A generic, format-agnostic view of a parsed binary file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Node {
    /// A user-defined kaitai type with named fields, in declaration order.
    Struct { ty: String, fields: Vec<Field> },
    /// A `repeat`-ed field: a homogeneous sequence of nodes.
    List { ty: String, items: Vec<Node> },
    UInt(u64),
    Int(i64),
    Float(f64),
    Str(String),
    /// Raw bytes (magic, opaque blobs, sized byte arrays).
    Bytes(Vec<u8>),
    /// A kaitai `enum` value: the symbolic name plus its numeric value.
    Enum { ty: String, name: String, value: i64 },
    /// An absent optional/conditional field, or an unmatched `switch`.
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub value: Node,
}

impl Field {
    pub fn new(name: impl Into<String>, value: Node) -> Self {
        Field { name: name.into(), value }
    }
}

impl Node {
    pub fn uint(v: impl Into<u64>) -> Node {
        Node::UInt(v.into())
    }
    pub fn int(v: impl Into<i64>) -> Node {
        Node::Int(v.into())
    }
    pub fn float(v: impl Into<f64>) -> Node {
        Node::Float(v.into())
    }
    pub fn enom(ty: impl Into<String>, name: impl Into<String>, value: i64) -> Node {
        Node::Enum { ty: ty.into(), name: name.into(), value }
    }

    /// A short label used by the diff renderer to identify a node's variant.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Node::Struct { .. } => "struct",
            Node::List { .. } => "list",
            Node::UInt(_) => "uint",
            Node::Int(_) => "int",
            Node::Float(_) => "float",
            Node::Str(_) => "str",
            Node::Bytes(_) => "bytes",
            Node::Enum { .. } => "enum",
            Node::Null => "null",
        }
    }

    /// Stable structural ("merkle") hash of this subtree. Equal subtrees share a
    /// hash, letting the differ skip identical children in O(1). Deterministic
    /// FNV-1a so results never depend on process-random seeds.
    pub fn content_hash(&self) -> u64 {
        let mut h = Fnv::new();
        self.hash_into(&mut h);
        h.0
    }

    fn hash_into(&self, h: &mut Fnv) {
        match self {
            Node::Struct { ty, fields } => {
                h.byte(1);
                h.str(ty);
                for f in fields {
                    h.str(&f.name);
                    f.value.hash_into(h);
                }
            }
            Node::List { ty, items } => {
                h.byte(2);
                h.str(ty);
                for it in items {
                    it.hash_into(h);
                }
            }
            Node::UInt(v) => {
                h.byte(3);
                h.bytes(&v.to_le_bytes());
            }
            Node::Int(v) => {
                h.byte(4);
                h.bytes(&v.to_le_bytes());
            }
            Node::Float(v) => {
                h.byte(5);
                h.bytes(&v.to_bits().to_le_bytes());
            }
            Node::Str(s) => {
                h.byte(6);
                h.str(s);
            }
            Node::Bytes(b) => {
                h.byte(7);
                h.bytes(b);
            }
            Node::Enum { ty, name, value } => {
                h.byte(8);
                h.str(ty);
                h.str(name);
                h.bytes(&value.to_le_bytes());
            }
            Node::Null => h.byte(9),
        }
    }
}

struct Fnv(u64);
impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf29ce484222325)
    }
    fn byte(&mut self, b: u8) {
        self.0 ^= b as u64;
        self.0 = self.0.wrapping_mul(0x100000001b3);
    }
    fn bytes(&mut self, bs: &[u8]) {
        for &b in bs {
            self.byte(b);
        }
    }
    fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
        self.byte(0);
    }
}

/// The envelope a plugin returns: either a parsed tree or a parse error string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParseResult {
    Ok(Node),
    Err(String),
}

impl ParseResult {
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|e| {
            serde_json::to_vec(&ParseResult::Err(format!("serialize error: {e}"))).unwrap()
        })
    }
    pub fn from_json_slice(bytes: &[u8]) -> Result<ParseResult, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("deserialize error: {e}"))
    }
}
