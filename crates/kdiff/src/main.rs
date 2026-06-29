//! kdiff: semantic binary diffing for git, driven by kaitai struct schemas.

mod codegen;
mod config;
mod diff;
mod model;
mod paths;
mod plugin;
mod rustkw;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use diff::DiffOptions;
use kdiff_abi::Node;
use plugin::{InstallOptions, LoadedPlugin};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "kdiff", version, about = "Semantic binary diffing via kaitai schemas")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install a schema from a URL or local path, compiling it to a plugin.
    Install {
        /// http(s) URL or local path to a .ksy file.
        source: String,
        /// Plugin name (defaults to the schema's meta/id).
        #[arg(long)]
        name: Option<String>,
        /// Also associate these file extensions with the schema.
        #[arg(long = "ext", value_delimiter = ',')]
        ext: Vec<String>,
        /// Also associate this hex magic prefix (e.g. 89504e47).
        #[arg(long)]
        magic: Option<String>,
        /// Build in debug profile (faster build, slower parse).
        #[arg(long)]
        dev: bool,
    },
    /// List installed plugins.
    List,
    /// Remove an installed plugin.
    Remove { name: String },
    /// Inspect or edit file-type associations.
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Diff two files semantically.
    Diff {
        old: PathBuf,
        new: PathBuf,
        /// Force a specific schema/plugin instead of auto-detecting.
        #[arg(long)]
        schema: Option<String>,
        /// when to colorize: auto, always, never.
        #[arg(long, default_value = "auto")]
        color: String,
    },
    /// Print a file's parsed tree (debugging aid).
    Show {
        file: PathBuf,
        #[arg(long)]
        schema: Option<String>,
    },
    /// GIT_EXTERNAL_DIFF / diff-driver entry: path old new ... (7 args).
    #[command(name = "git-diff", hide = true)]
    GitDiff { args: Vec<String> },
    /// Configure git to use kdiff as an external diff driver.
    InstallGit {
        /// Configure globally instead of in the current repo.
        #[arg(long)]
        global: bool,
    },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Show the current config.
    Show,
    /// Associate a schema with extensions and/or a magic prefix.
    Associate {
        schema: String,
        #[arg(long = "ext", value_delimiter = ',')]
        ext: Vec<String>,
        #[arg(long)]
        magic: Option<String>,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("kdiff: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
        Cmd::Install { source, name, ext, magic, dev } => cmd_install(source, name, ext, magic, dev),
        Cmd::List => cmd_list(),
        Cmd::Remove { name } => plugin::remove(&name).map(|_| println!("removed {name}")),
        Cmd::Config { action } => cmd_config(action),
        Cmd::Diff { old, new, schema, color } => cmd_diff(&old, &new, schema, &color),
        Cmd::Show { file, schema } => cmd_show(&file, schema),
        Cmd::GitDiff { args } => cmd_git_diff(args),
        Cmd::InstallGit { global } => cmd_install_git(global),
    }
}

fn cmd_install(
    source: String,
    name: Option<String>,
    ext: Vec<String>,
    magic: Option<String>,
    dev: bool,
) -> Result<()> {
    let mut opts = InstallOptions { name, ..Default::default() };
    if dev {
        opts.profile = "dev".to_string();
    }
    eprintln!("Installing schema from {source} ...");
    let meta = plugin::install(&source, &opts)?;
    println!("Installed plugin `{}` (schema id `{}`)", meta.name, meta.schema_id);

    if !ext.is_empty() || magic.is_some() {
        let mut cfg = Config::load()?;
        cfg.associate(&meta.name, &ext, magic);
        cfg.save()?;
        println!("Associated `{}` with {:?}", meta.name, ext);
    }
    Ok(())
}

fn cmd_list() -> Result<()> {
    let plugins = plugin::list_installed()?;
    if plugins.is_empty() {
        println!("No plugins installed. Try: kdiff install <url-or-path.ksy>");
        return Ok(());
    }
    for p in plugins {
        println!("{:<16} schema={:<12} abi=v{} source={}", p.name, p.schema_id, p.abi_version, p.source);
    }
    Ok(())
}

fn cmd_config(action: ConfigCmd) -> Result<()> {
    match action {
        ConfigCmd::Show => {
            let cfg = Config::load()?;
            print!("{}", toml::to_string_pretty(&cfg)?);
            Ok(())
        }
        ConfigCmd::Associate { schema, ext, magic } => {
            let mut cfg = Config::load()?;
            cfg.associate(&schema, &ext, magic);
            cfg.save()?;
            println!("Updated associations for `{schema}`");
            Ok(())
        }
    }
}

/// Resolve which schema to use for a file given an optional override.
fn resolve_schema(cfg: &Config, path: &Path, head: &[u8], override_: &Option<String>) -> Result<String> {
    if let Some(s) = override_ {
        return Ok(s.clone());
    }
    cfg.detect(path, head).with_context(|| {
        format!(
            "no schema associated for {}; pass --schema NAME or run `kdiff config associate`",
            path.display()
        )
    })
}

fn cmd_diff(old: &Path, new: &Path, schema: Option<String>, color: &str) -> Result<()> {
    let old_bytes = std::fs::read(old).with_context(|| format!("reading {}", old.display()))?;
    let new_bytes = std::fs::read(new).with_context(|| format!("reading {}", new.display()))?;

    // Fast path: byte-identical files have no diff (skips parsing entirely).
    if old_bytes == new_bytes {
        return Ok(());
    }

    let cfg = Config::load()?;
    let head: &[u8] = if !new_bytes.is_empty() { &new_bytes } else { &old_bytes };
    let name = resolve_schema(&cfg, new, head, &schema)?;
    let plugin = LoadedPlugin::load(&name)?;

    let told = plugin.parse_bytes(&old_bytes)?;
    let tnew = plugin.parse_bytes(&new_bytes)?;

    let use_color = want_color(color);
    let text = diff::diff(&told, &tnew, &DiffOptions { color: use_color });
    emit(&text)
}

fn cmd_show(file: &Path, schema: Option<String>) -> Result<()> {
    let bytes = std::fs::read(file)?;
    let cfg = Config::load()?;
    let name = resolve_schema(&cfg, file, &bytes, &schema)?;
    let plugin = LoadedPlugin::load(&name)?;
    let tree = plugin.parse_bytes(&bytes)?;
    print_tree(&tree, "", 0);
    Ok(())
}

fn print_tree(node: &Node, label: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    match node {
        Node::Struct { ty, fields } => {
            println!("{indent}{label}{}{ty}", if label.is_empty() { "" } else { ": " });
            for f in fields {
                print_tree(&f.value, &f.name, depth + 1);
            }
        }
        Node::List { items, .. } => {
            println!("{indent}{label}: [{} items]", items.len());
            for (i, it) in items.iter().enumerate() {
                print_tree(it, &format!("[{i}]"), depth + 1);
            }
        }
        other => println!("{indent}{label}: {}", scalar_show(other)),
    }
}

fn scalar_show(n: &Node) -> String {
    match n {
        Node::UInt(v) => v.to_string(),
        Node::Int(v) => v.to_string(),
        Node::Float(v) => v.to_string(),
        Node::Str(s) => format!("{s:?}"),
        Node::Bytes(b) => format!("<{} bytes>", b.len()),
        Node::Enum { name, value, .. } => format!("{name}({value})"),
        Node::Null => "null".to_string(),
        _ => String::new(),
    }
}

/// External-diff entry. Git passes: path old-file old-hex old-mode new-file new-hex new-mode.
fn cmd_git_diff(args: Vec<String>) -> Result<()> {
    if args.len() < 5 {
        bail!("git-diff expects git's external-diff arguments");
    }
    let path = Path::new(&args[0]);
    let old_file = Path::new(&args[1]);
    let new_file = Path::new(&args[4]);

    let old_bytes = read_or_empty(old_file);
    let new_bytes = read_or_empty(new_file);

    println!("kdiff {}:", path.display());
    if old_bytes == new_bytes {
        return Ok(());
    }

    let cfg = Config::load()?;
    let head: &[u8] = if !new_bytes.is_empty() { &new_bytes } else { &old_bytes };
    let name = match resolve_schema(&cfg, path, head, &None) {
        Ok(n) => n,
        Err(e) => {
            // Fall back gracefully so git still shows *something*.
            println!("(no kdiff schema: {e})");
            return Ok(());
        }
    };
    let plugin = LoadedPlugin::load(&name)?;
    let use_color = std::io::stdout().is_terminal();
    let opts = DiffOptions { color: use_color };

    let text = match (old_bytes.is_empty(), new_bytes.is_empty()) {
        (true, false) => {
            let t = plugin.parse_bytes(&new_bytes)?;
            full_oneway(&t, '+', use_color)
        }
        (false, true) => {
            let t = plugin.parse_bytes(&old_bytes)?;
            full_oneway(&t, '-', use_color)
        }
        _ => {
            let told = plugin.parse_bytes(&old_bytes)?;
            let tnew = plugin.parse_bytes(&new_bytes)?;
            diff::diff(&told, &tnew, &opts)
        }
    };
    print!("{text}");
    Ok(())
}

/// Render a whole tree one-directionally (new file added / old file removed).
fn full_oneway(tree: &Node, sign: char, color: bool) -> String {
    let empty = Node::Null;
    let (old, new) = if sign == '+' { (&empty, tree) } else { (tree, &empty) };
    diff::diff(old, new, &DiffOptions { color })
}

fn read_or_empty(p: &Path) -> Vec<u8> {
    if p == Path::new("/dev/null") {
        return Vec::new();
    }
    std::fs::read(p).unwrap_or_default()
}

fn cmd_install_git(global: bool) -> Result<()> {
    let exe = std::env::current_exe()?;
    let cmdline = format!("{} git-diff", exe.display());
    let scope = if global { "--global" } else { "--local" };
    run_git(&["config", scope, "diff.kdiff.command", &cmdline])?;
    println!("Configured git diff driver `kdiff`.");
    println!("Now associate file types in .gitattributes, e.g.:");
    println!("    *.png    diff=kdiff");
    println!("    *.sqlite diff=kdiff");
    Ok(())
}

fn run_git(args: &[&str]) -> Result<()> {
    let status = Command::new("git").args(args).status().context("running git")?;
    if !status.success() {
        bail!("git {:?} failed", args);
    }
    Ok(())
}

fn want_color(when: &str) -> bool {
    match when {
        "always" => true,
        "never" => false,
        _ => std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal(),
    }
}

/// Print diff text, paging through $PAGER (or `less -R`) when on a terminal.
fn emit(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    let to_tty = std::io::stdout().is_terminal();
    let paging = to_tty && std::env::var_os("KDIFF_NO_PAGER").is_none();
    if paging {
        let pager = std::env::var("PAGER").unwrap_or_else(|_| "less -R".to_string());
        let mut parts = pager.split_whitespace();
        if let Some(prog) = parts.next() {
            if let Ok(mut child) = Command::new(prog).args(parts).stdin(Stdio::piped()).spawn() {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                let _ = child.wait();
                return Ok(());
            }
        }
    }
    print!("{text}");
    Ok(())
}
