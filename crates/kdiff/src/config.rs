//! User-level config: maps file types (by extension and/or magic prefix) to an
//! installed schema/plugin name. Stored as TOML at `config.toml`.

use crate::paths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, rename = "association")]
    pub associations: Vec<Association>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Association {
    /// Installed plugin/schema name to use for this file type.
    pub schema: String,
    /// Lower-case file extensions (without the dot).
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Hex-encoded magic byte prefix, e.g. "89504e47" for PNG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magic: Option<String>,
}

impl Config {
    pub fn load() -> Result<Config> {
        let path = paths::config_file()?;
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let dir = paths::config_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = paths::config_file()?;
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&path, text).with_context(|| format!("writing config {}", path.display()))?;
        Ok(())
    }

    /// Add or update the association for `schema`, merging extensions/magic.
    pub fn associate(&mut self, schema: &str, extensions: &[String], magic: Option<String>) {
        if let Some(a) = self.associations.iter_mut().find(|a| a.schema == schema) {
            for e in extensions {
                let e = e.to_lowercase();
                if !a.extensions.contains(&e) {
                    a.extensions.push(e);
                }
            }
            if magic.is_some() {
                a.magic = magic;
            }
        } else {
            self.associations.push(Association {
                schema: schema.to_string(),
                extensions: extensions.iter().map(|e| e.to_lowercase()).collect(),
                magic,
            });
        }
    }

    /// Resolve the schema name for a file, preferring a magic-prefix match over
    /// an extension match (magic is more reliable than a possibly-wrong name).
    pub fn detect(&self, path: &Path, head: &[u8]) -> Option<String> {
        if let Some(a) = self.associations.iter().find(|a| a.magic_matches(head)) {
            return Some(a.schema.clone());
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        if let Some(ext) = ext {
            if let Some(a) = self.associations.iter().find(|a| a.extensions.contains(&ext)) {
                return Some(a.schema.clone());
            }
        }
        None
    }
}

impl Association {
    fn magic_matches(&self, head: &[u8]) -> bool {
        let Some(magic) = &self.magic else { return false };
        match hex_decode(magic) {
            Some(bytes) if !bytes.is_empty() => head.starts_with(&bytes),
            _ => false,
        }
    }
}

pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for pair in bytes.chunks(2) {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
    }
    Some(out)
}
