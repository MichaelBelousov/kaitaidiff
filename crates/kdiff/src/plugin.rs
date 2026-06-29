//! Installing and loading schema plugins.
//!
//! Install: fetch a `.ksy`, run `kaitai-struct-compiler` to generate the Rust
//! parser, generate a walker `lib.rs` from our model, assemble a `cdylib` crate,
//! `cargo build` it (sharing a target cache), and store the resulting shared
//! object plus metadata under the data dir.
//!
//! Load: `dlopen` the shared object, check the ABI version, and call the C-ABI
//! `kdiff_parse` to turn a byte buffer into a [`Node`] tree.

use crate::{codegen, model, paths};
use anyhow::{bail, Context, Result};
use kdiff_abi::{Node, ParseResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub schema_id: String,
    pub root_rust: String,
    pub abi_version: u32,
    pub source: String,
    /// Shared-object filename within the plugin dir.
    pub lib_file: String,
}

impl PluginMeta {
    pub fn lib_path(&self) -> Result<PathBuf> {
        Ok(paths::plugin_dir(&self.name)?.join(&self.lib_file))
    }
}

pub fn meta_path(name: &str) -> Result<PathBuf> {
    Ok(paths::plugin_dir(name)?.join("meta.json"))
}

pub fn load_meta(name: &str) -> Result<PluginMeta> {
    let p = meta_path(name)?;
    let text = std::fs::read_to_string(&p)
        .with_context(|| format!("plugin `{name}` not installed ({})", p.display()))?;
    Ok(serde_json::from_str(&text)?)
}

pub fn list_installed() -> Result<Vec<PluginMeta>> {
    let dir = paths::plugins_dir()?;
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(m) = load_meta(name) {
                    out.push(m);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn remove(name: &str) -> Result<()> {
    let dir = paths::plugin_dir(name)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// Fetch the raw `.ksy` bytes from an http(s) URL or a local path.
pub fn fetch(source: &str) -> Result<Vec<u8>> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let out = Command::new("curl")
            .args(["-fsSL", source])
            .output()
            .context("failed to run curl (is it installed?)")?;
        if !out.status.success() {
            bail!("curl failed for {source}: {}", String::from_utf8_lossy(&out.stderr));
        }
        Ok(out.stdout)
    } else {
        let path = source.strip_prefix("file://").unwrap_or(source);
        std::fs::read(path).with_context(|| format!("reading schema file {path}"))
    }
}

pub struct InstallOptions {
    pub name: Option<String>,
    /// "release" (default) or "dev".
    pub profile: String,
}

impl Default for InstallOptions {
    fn default() -> Self {
        let profile = std::env::var("KDIFF_BUILD_PROFILE").unwrap_or_else(|_| "release".into());
        InstallOptions { name: None, profile }
    }
}

pub fn install(source: &str, opts: &InstallOptions) -> Result<PluginMeta> {
    let ksy_bytes = fetch(source)?;
    let ksy_text = String::from_utf8(ksy_bytes.clone()).context("schema is not valid UTF-8")?;
    let schema = model::parse_schema(&ksy_text)?;
    let name = opts.name.clone().unwrap_or_else(|| schema.id.clone());

    let crate_name = format!("kdiff_plugin_{}", sanitize(&name));
    let crate_dir = paths::build_dir()?.join(&name);
    let src_dir = crate_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // 1. Persist the .ksy and run the kaitai compiler into the crate's src.
    let ksy_path = crate_dir.join("schema.ksy");
    std::fs::write(&ksy_path, &ksy_text)?;
    run_ksc(&ksy_path, &src_dir)?;
    // ksc names the file after meta.id; normalize to schema.rs.
    let generated = src_dir.join(format!("{}.rs", schema.id));
    let schema_rs = src_dir.join("schema.rs");
    if generated.exists() {
        std::fs::rename(&generated, &schema_rs)?;
    } else if !schema_rs.exists() {
        bail!("kaitai compiler did not produce {}", generated.display());
    }
    // Patch the kaitai-rust keyword-escaping bug (e.g. a field named `type`).
    let patched = crate::rustkw::escape_rust_keywords(&std::fs::read_to_string(&schema_rs)?);
    std::fs::write(&schema_rs, patched)?;

    // 2. Generate the walker lib.rs + Cargo.toml.
    std::fs::write(src_dir.join("lib.rs"), codegen::generate_lib_rs(&schema))?;
    std::fs::write(crate_dir.join("Cargo.toml"), plugin_cargo_toml(&crate_name)?)?;

    // 3. Compile the cdylib, reusing a shared target cache.
    let target_dir = paths::build_cache_dir()?;
    std::fs::create_dir_all(&target_dir)?;
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&crate_dir)
        .arg("build")
        .env("CARGO_TARGET_DIR", &target_dir);
    let profile_subdir = if opts.profile == "release" {
        cmd.arg("--release");
        "release"
    } else {
        "debug"
    };
    let status = cmd.status().context("failed to run cargo")?;
    if !status.success() {
        bail!("cargo build failed for plugin `{name}`");
    }

    // 4. Copy the shared object + metadata into the plugin dir.
    let lib_file = shared_lib_filename(&crate_name);
    let built = target_dir.join(profile_subdir).join(&lib_file);
    if !built.exists() {
        bail!("expected built library not found: {}", built.display());
    }
    let dest_dir = paths::plugin_dir(&name)?;
    std::fs::create_dir_all(&dest_dir)?;
    std::fs::copy(&built, dest_dir.join(&lib_file))?;
    std::fs::copy(&ksy_path, dest_dir.join("schema.ksy"))?;

    let meta = PluginMeta {
        name: name.clone(),
        schema_id: schema.id.clone(),
        root_rust: schema.root_rust.clone(),
        abi_version: kdiff_abi::ABI_VERSION,
        source: source.to_string(),
        lib_file,
    };
    std::fs::write(meta_path(&name)?, serde_json::to_string_pretty(&meta)?)?;
    Ok(meta)
}

fn run_ksc(ksy: &Path, out_dir: &Path) -> Result<()> {
    let out = Command::new("kaitai-struct-compiler")
        .args(["-t", "rust", "--outdir"])
        .arg(out_dir)
        .arg(ksy)
        .output()
        .context("failed to run kaitai-struct-compiler (is it installed?)")?;
    if !out.status.success() {
        bail!(
            "kaitai-struct-compiler failed:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

fn plugin_cargo_toml(crate_name: &str) -> Result<String> {
    let kaitai = paths::kaitai_runtime_path()?;
    let abi = paths::kdiff_abi_path()?;
    Ok(format!(
        "[package]\n\
         name = \"{crate_name}\"\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\
         publish = false\n\n\
         [lib]\n\
         crate-type = [\"cdylib\"]\n\n\
         [dependencies]\n\
         kaitai = {{ path = {kaitai:?} }}\n\
         kdiff-abi = {{ path = {abi:?} }}\n",
        kaitai = kaitai.display().to_string(),
        abi = abi.display().to_string(),
    ))
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn shared_lib_filename(crate_name: &str) -> String {
    let stem = crate_name.replace('-', "_");
    if cfg!(target_os = "windows") {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

type AbiVersionFn = unsafe extern "C" fn() -> u32;
type ParseFn = unsafe extern "C" fn(*const u8, usize, *mut usize) -> *mut u8;
type FreeFn = unsafe extern "C" fn(*mut u8, usize);

pub struct LoadedPlugin {
    _lib: libloading::Library,
    parse: ParseFn,
    free: FreeFn,
    #[allow(dead_code)]
    pub meta: PluginMeta,
}

impl LoadedPlugin {
    pub fn load(name: &str) -> Result<LoadedPlugin> {
        let meta = load_meta(name)?;
        let lib_path = meta.lib_path()?;
        unsafe {
            let lib = libloading::Library::new(&lib_path)
                .with_context(|| format!("dlopen {}", lib_path.display()))?;
            let abi: libloading::Symbol<AbiVersionFn> = lib
                .get(b"kdiff_abi_version")
                .context("plugin missing kdiff_abi_version")?;
            let got = abi();
            if got != kdiff_abi::ABI_VERSION {
                bail!(
                    "plugin `{name}` ABI v{got} incompatible with kdiff ABI v{} — reinstall it",
                    kdiff_abi::ABI_VERSION
                );
            }
            let parse: libloading::Symbol<ParseFn> =
                lib.get(b"kdiff_parse").context("plugin missing kdiff_parse")?;
            let free: libloading::Symbol<FreeFn> =
                lib.get(b"kdiff_free").context("plugin missing kdiff_free")?;
            let parse = *parse;
            let free = *free;
            Ok(LoadedPlugin { _lib: lib, parse, free, meta })
        }
    }

    pub fn parse_bytes(&self, bytes: &[u8]) -> Result<Node> {
        let mut out_len: usize = 0;
        let ptr = unsafe { (self.parse)(bytes.as_ptr(), bytes.len(), &mut out_len) };
        if ptr.is_null() {
            bail!("plugin returned null");
        }
        let json = unsafe { std::slice::from_raw_parts(ptr, out_len) }.to_vec();
        unsafe { (self.free)(ptr, out_len) };
        match ParseResult::from_json_slice(&json).map_err(anyhow::Error::msg)? {
            ParseResult::Ok(node) => Ok(node),
            ParseResult::Err(e) => bail!("parse error: {e}"),
        }
    }
}
