//! Parse a `.ksy` schema into a resolved model that the walker codegen consumes.
//!
//! We deliberately do **not** evaluate kaitai expressions — the kaitai-generated
//! parser does that at runtime. We only need each field's *name*, *declared kind*,
//! and whether it `repeat`s, so we can emit the matching getter call and recurse.

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Raw YAML shape (only the keys we care about).
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct RawKsy {
    meta: RawMeta,
    #[serde(default)]
    seq: Vec<RawAttr>,
    #[serde(default)]
    types: BTreeMap<String, RawType>,
    #[serde(default)]
    enums: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, serde::Deserialize)]
struct RawMeta {
    id: String,
    // endian/encoding etc. affect parsing (done by kaitai), not walking.
    #[serde(default)]
    #[allow(dead_code)]
    endian: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct RawType {
    #[serde(default)]
    seq: Vec<RawAttr>,
    #[serde(default)]
    types: BTreeMap<String, RawType>,
    #[serde(default)]
    enums: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, serde::Deserialize)]
struct RawAttr {
    id: Option<String>,
    #[serde(default, rename = "type")]
    ty: Option<TypeSpec>,
    #[serde(default)]
    #[allow(dead_code)]
    contents: Option<serde_yaml::Value>,
    #[serde(default)]
    repeat: Option<String>,
    #[serde(default, rename = "enum")]
    enom: Option<String>,
    // size / if / repeat-expr / encoding etc. are intentionally ignored: they
    // affect parsing (done by kaitai) but not how we walk the typed result.
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum TypeSpec {
    Name(String),
    Switch(SwitchSpec),
}

#[derive(Debug, serde::Deserialize)]
struct SwitchSpec {
    #[serde(rename = "switch-on")]
    #[allow(dead_code)]
    switch_on: serde_yaml::Value,
    cases: BTreeMap<String, String>,
}

// ---------------------------------------------------------------------------
// Resolved model.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Schema {
    pub id: String,
    pub root_rust: String,
    pub types: Vec<WType>,
}

#[derive(Debug, Clone)]
pub struct WType {
    pub rust_name: String,
    pub fields: Vec<WField>,
}

#[derive(Debug, Clone)]
pub struct WField {
    pub id: String,
    pub kind: Kind,
    pub repeat: bool,
}

#[derive(Debug, Clone)]
pub enum Kind {
    UInt,
    Int,
    Float,
    Str,
    Bytes,
    Enum { rust_name: String },
    User { rust_name: String },
    Switch { enum_name: String, variants: Vec<SwitchVariant> },
}

#[derive(Debug, Clone)]
pub struct SwitchVariant {
    /// The generated enum variant identifier, e.g. `Demo_TextBody`.
    pub variant: String,
    /// The case type's walker function target, same string as `variant`.
    pub case_rust: String,
}

/// Convert a kaitai snake_case id into UpperCamelCase: `text_body` -> `TextBody`.
fn camel(id: &str) -> String {
    id.split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// The Rust type name kaitai emits for a type at `path` (segments are ids from
/// root downward): each segment camel-cased, joined by `_`. Root => just itself.
fn rust_name(path: &[String]) -> String {
    path.iter().map(|s| camel(s)).collect::<Vec<_>>().join("_")
}

pub fn parse_schema(yaml: &str) -> Result<Schema> {
    let raw: RawKsy = serde_yaml::from_str(yaml).context("invalid .ksy YAML")?;
    let root_id = raw.meta.id.clone();
    let root_path = vec![root_id.clone()];

    // Pass 1: collect rust names of every type and enum (assumes globally unique
    // ids, which holds for the formats we target; documented limitation).
    let mut type_names: BTreeMap<String, String> = BTreeMap::new();
    let mut enum_names: BTreeMap<String, String> = BTreeMap::new();
    collect_names(&root_id, &raw.types, &raw.enums, &root_path, &mut type_names, &mut enum_names);

    // Pass 2: build walker types.
    let mut out = Vec::new();
    build_type(&root_path, &raw.seq, &type_names, &enum_names, &mut out)?;
    collect_types(&raw.types, &root_path, &type_names, &enum_names, &mut out)?;

    Ok(Schema { id: root_id, root_rust: rust_name(&root_path), types: out })
}

fn collect_names(
    _root_id: &str,
    types: &BTreeMap<String, RawType>,
    enums: &BTreeMap<String, serde_yaml::Value>,
    path: &[String],
    type_names: &mut BTreeMap<String, String>,
    enum_names: &mut BTreeMap<String, String>,
) {
    for (id, t) in types {
        let mut p = path.to_vec();
        p.push(id.clone());
        type_names.insert(id.clone(), rust_name(&p));
        collect_names(_root_id, &t.types, &t.enums, &p, type_names, enum_names);
    }
    for id in enums.keys() {
        let mut p = path.to_vec();
        p.push(id.clone());
        enum_names.insert(id.clone(), rust_name(&p));
    }
}

fn collect_types(
    types: &BTreeMap<String, RawType>,
    path: &[String],
    type_names: &BTreeMap<String, String>,
    enum_names: &BTreeMap<String, String>,
    out: &mut Vec<WType>,
) -> Result<()> {
    for (id, t) in types {
        let mut p = path.to_vec();
        p.push(id.clone());
        build_type(&p, &t.seq, type_names, enum_names, out)?;
        collect_types(&t.types, &p, type_names, enum_names, out)?;
    }
    Ok(())
}

fn build_type(
    path: &[String],
    seq: &[RawAttr],
    type_names: &BTreeMap<String, String>,
    enum_names: &BTreeMap<String, String>,
    out: &mut Vec<WType>,
) -> Result<()> {
    let owner_rust = rust_name(path);
    let mut fields = Vec::new();
    for attr in seq {
        let Some(id) = &attr.id else { continue };
        let kind = classify(&owner_rust, id, attr, type_names, enum_names)?;
        fields.push(WField { id: id.clone(), kind, repeat: attr.repeat.is_some() });
    }
    out.push(WType { rust_name: owner_rust, fields });
    Ok(())
}

fn classify(
    owner_rust: &str,
    field_id: &str,
    attr: &RawAttr,
    type_names: &BTreeMap<String, String>,
    enum_names: &BTreeMap<String, String>,
) -> Result<Kind> {
    // enum overrides the numeric base type.
    if let Some(enom) = &attr.enom {
        let rn = enum_names
            .get(enom)
            .cloned()
            .with_context(|| format!("unknown enum `{enom}` on field `{field_id}`"))?;
        return Ok(Kind::Enum { rust_name: rn });
    }

    match &attr.ty {
        None => {
            // no type: magic `contents` or a sized/eos byte array -> raw bytes.
            Ok(Kind::Bytes)
        }
        Some(TypeSpec::Name(name)) => classify_named(name, type_names),
        Some(TypeSpec::Switch(sw)) => {
            let enum_name = format!("{owner_rust}_{}", camel(field_id));
            let mut seen = BTreeMap::new();
            for case_ty in sw.cases.values() {
                let rn = type_names.get(case_ty).cloned().with_context(|| {
                    format!("switch case type `{case_ty}` on `{field_id}` is not a user type (unsupported)")
                })?;
                seen.entry(rn.clone()).or_insert(SwitchVariant { variant: rn.clone(), case_rust: rn });
            }
            Ok(Kind::Switch { enum_name, variants: seen.into_values().collect() })
        }
    }
}

fn classify_named(name: &str, type_names: &BTreeMap<String, String>) -> Result<Kind> {
    if let Some(k) = builtin_kind(name) {
        return Ok(k);
    }
    if let Some(rn) = type_names.get(name) {
        return Ok(Kind::User { rust_name: rn.clone() });
    }
    bail!("unknown type `{name}` (not a builtin or defined user type)")
}

/// Classify a kaitai builtin scalar type name.
fn builtin_kind(name: &str) -> Option<Kind> {
    match name {
        "str" | "strz" => return Some(Kind::Str),
        "f4" | "f8" | "f4be" | "f4le" | "f8be" | "f8le" => return Some(Kind::Float),
        _ => {}
    }
    // u1/u2/u4/u8 (+le/be), s1.. -> integers; b1.. bit ints -> unsigned.
    let bytes_endian = |s: &str| -> bool {
        let core = s.trim_end_matches("le").trim_end_matches("be");
        matches!(core, "1" | "2" | "4" | "8")
    };
    if let Some(rest) = name.strip_prefix('u') {
        if bytes_endian(rest) {
            return Some(Kind::UInt);
        }
    }
    if let Some(rest) = name.strip_prefix('s') {
        if bytes_endian(rest) {
            return Some(Kind::Int);
        }
    }
    if let Some(rest) = name.strip_prefix('b') {
        if rest.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty() {
            return Some(Kind::UInt);
        }
    }
    None
}
