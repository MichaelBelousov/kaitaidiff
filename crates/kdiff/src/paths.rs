//! Resolves the on-disk locations kdiff uses, all overridable via environment
//! variables so tests can run fully sandboxed.
//!
//! * `KDIFF_DATA_DIR`    — plugins, build cache (default `~/.local/share/kdiff`)
//! * `KDIFF_CONFIG_DIR`  — `config.toml`          (default `~/.config/kdiff`)
//! * `KDIFF_SUPPORT_DIR` — checkout providing the `kaitai` runtime + `kdiff-abi`
//!   crates that compiled plugins depend on (default `<data>/support`).

use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn data_dir() -> Result<PathBuf> {
    if let Ok(d) = std::env::var("KDIFF_DATA_DIR") {
        return Ok(PathBuf::from(d));
    }
    let base = dirs::data_dir().context("cannot determine user data dir")?;
    Ok(base.join("kdiff"))
}

pub fn config_dir() -> Result<PathBuf> {
    if let Ok(d) = std::env::var("KDIFF_CONFIG_DIR") {
        return Ok(PathBuf::from(d));
    }
    let base = dirs::config_dir().context("cannot determine user config dir")?;
    Ok(base.join("kdiff"))
}

pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn plugins_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("plugins"))
}

pub fn plugin_dir(name: &str) -> Result<PathBuf> {
    Ok(plugins_dir()?.join(name))
}

/// Shared cargo target dir so each plugin build reuses compiled deps.
pub fn build_cache_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("build-cache"))
}

/// Where plugin crates are generated and compiled.
pub fn build_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("build"))
}

/// Checkout providing the `kaitai` runtime and `kdiff-abi` path dependencies.
pub fn support_dir() -> Result<PathBuf> {
    if let Ok(d) = std::env::var("KDIFF_SUPPORT_DIR") {
        return Ok(PathBuf::from(d));
    }
    Ok(data_dir()?.join("support"))
}

pub fn kaitai_runtime_path() -> Result<PathBuf> {
    Ok(support_dir()?.join("vendor/kaitai_struct_rust_runtime"))
}

pub fn kdiff_abi_path() -> Result<PathBuf> {
    Ok(support_dir()?.join("crates/kdiff-abi"))
}
