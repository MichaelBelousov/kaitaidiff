# kdiff — semantic binary diffing for git, driven by kaitai struct schemas

`kdiff` diffs binary files by their *structure* instead of their bytes. You give
it a [kaitai struct](https://kaitai.io) `.ksy` schema for a format; it compiles
that schema into a fast native parser, parses both files into a tree, and shows
a structural (tree) diff — so a one-pixel resize of a PNG reads as
`width: 1 → 2`, not a wall of changed bytes.

```
$ kdiff diff old.png new.png
~ Png (Png)
~   ihdr (Png_IhdrChunk)
-     width: 1
+     width: 2
```

## How it works

```
 install:   GET schema.ksy ──▶ kaitai-struct-compiler -t rust ──▶ schema.rs
                                        │
                       generate walker lib.rs (Node tree + C ABI)
                                        │
                       cargo build --release  ──▶  libkdiff_plugin_<name>.so
                                        │
                          ~/.local/share/kdiff/plugins/<name>/

 diff:      detect type (magic/ext) ─▶ dlopen plugin ─▶ kdiff_parse(bytes)
            ─▶ Node tree (JSON over the ABI) ─▶ structural tree-diff ─▶ output
```

* **Compiled, not interpreted.** Each schema becomes a monomorphized native
  parser via kaitai's Rust backend, so parsing large files stays fast. The
  generated parser is wrapped by a **codegen'd walker** (`crates/kdiff/src/codegen.rs`)
  that turns the strongly-typed kaitai struct into a generic [`Node`] tree —
  kaitai's generated Rust exposes no reflection, so the walker is generated
  per-schema from our own parse of the `.ksy`.
* **Plugins are `dlopen`ed.** Plugins talk to the CLI over a tiny, versioned C
  ABI (`kdiff-abi`): `kdiff_parse` / `kdiff_free` / `kdiff_abi_version`.
* **Structural diff.** Every tree node carries a deterministic merkle
  (FNV-1a) hash, so identical subtrees are skipped in O(1). Struct fields diff
  positionally; repeated fields diff via LCS, pairing adjacent delete/insert
  runs into recursive "modified element" diffs.
* **Identical prefixes/suffixes** are short-circuited: byte-identical files (and
  trees with equal root hashes) produce no output without descending.

## Install

```sh
# build the CLI
cargo build --release            # target/release/kdiff

# the plugin compiler needs the kaitai runtime + abi crates this repo vendors.
# Point kdiff at this checkout (or copy vendor/ + crates/kdiff-abi into the
# default ~/.local/share/kdiff/support/):
export KDIFF_SUPPORT_DIR=/path/to/this/repo
```

Requirements: a Rust toolchain, `kaitai-struct-compiler` (0.11) on `PATH`, and
`curl` for URL installs.

## Usage

```sh
# install a schema from a URL or local path, and associate file types
kdiff install https://example.com/png.ksy --ext png --magic 89504e47
kdiff install schemas/sqlite3.ksy --name sqlite --ext sqlite,db

kdiff list                       # installed plugins
kdiff config show                # file-type associations
kdiff config associate png --ext apng

kdiff diff a.png b.png           # auto-detects schema, pages like git
kdiff diff a.bin b.bin --schema myfmt
kdiff show a.png                 # dump the parsed tree
```

### Git integration

```sh
kdiff install-git                # registers the `kdiff` diff driver in git
echo '*.png    diff=kdiff' >> .gitattributes
echo '*.sqlite diff=kdiff' >> .gitattributes

git diff                         # binary changes now render structurally
```

`install-git` sets `diff.kdiff.command` to `kdiff git-diff`; git calls it with
its external-diff arguments. Output is paged by git as usual.

## Configuration & layout

| Path | Purpose | Override |
|------|---------|----------|
| `~/.config/kdiff/config.toml`   | file-type ↔ schema associations | `KDIFF_CONFIG_DIR` |
| `~/.local/share/kdiff/plugins/` | compiled plugins + metadata     | `KDIFF_DATA_DIR` |
| `~/.local/share/kdiff/build-cache/` | shared cargo target cache   | — |
| support checkout (`vendor/`, `crates/kdiff-abi`) | plugin build deps | `KDIFF_SUPPORT_DIR` |

`KDIFF_BUILD_PROFILE=dev` builds plugins faster (debug); `KDIFF_NO_PAGER=1`
disables paging.

## Known limitations

* **Instances** (kaitai `instances:`, including `pos`-based ones) are not walked
  yet — only `seq` fields are. This is the main thing standing between the
  header-level SQLite schema here and a full one.
* **Unique type ids** are assumed across a schema (true for the formats we
  target; nested types reusing an id in different scopes are not disambiguated).
* **Switch cases must be user types** (not builtins).
* **`if` on a primitive** renders the kaitai default when the condition is false
  (the Rust backend doesn't `Option`-wrap primitives); `if` on a user type is
  handled correctly (renders `null`).
* The Rust target of kaitai 0.11 doesn't escape field names that are Rust
  keywords (e.g. `type`); kdiff patches the generated source
  (`crates/kdiff/src/rustkw.rs`).

## Roadmap

* Walk `instances` so formats like full SQLite (logical b-tree pages → rows) and
  Revit work end to end.
* App-specific **semantic plugins** that present logical hierarchies (SQLite
  rows, Revit elements) instead of physical structure.
* Optionally extend the kaitai compiler to emit a merkle hash *during parsing*,
  so unchanged regions can be skipped before they're even materialized.

## Tests

```sh
cargo test                       # unit tests
cargo test --test e2e            # builds real plugins, diffs PNG + SQLite,
                                 # runs a git commit→stage→diff round-trip,
                                 # and installs a schema over HTTP
```
