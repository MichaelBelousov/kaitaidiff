//! End-to-end tests: install schemas (compiling real cdylib plugins), then diff
//! PNG and SQLite fixtures and assert exact text output, including a full git
//! commit -> stage -> diff round-trip.
//!
//! These build plugins with `cargo`, so they need `kaitai-struct-compiler` on
//! PATH and a working Rust toolchain. They share one home dir (and thus one
//! build cache) so the kaitai runtime is compiled only once.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, OnceLock};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn shared_home() -> &'static Path {
    static H: OnceLock<PathBuf> = OnceLock::new();
    H.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("kdiff-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    })
}

fn kdiff(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kdiff"))
        .args(args)
        .env("KDIFF_DATA_DIR", shared_home().join("data"))
        .env("KDIFF_CONFIG_DIR", shared_home().join("config"))
        .env("KDIFF_SUPPORT_DIR", workspace_root())
        .env("KDIFF_BUILD_PROFILE", "dev")
        .env("KDIFF_NO_PAGER", "1")
        .env("NO_COLOR", "1")
        .output()
        .expect("run kdiff")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Install a schema exactly once across all tests (guarded so parallel tests
/// don't double-build the same plugin).
fn ensure_installed(name: &str, ksy: &str, extra: &[&str]) {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _g = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let meta = shared_home().join("data/plugins").join(name).join("meta.json");
    if meta.exists() {
        return;
    }
    let ksy_path = workspace_root().join("schemas").join(ksy);
    let mut args = vec!["install", ksy_path.to_str().unwrap()];
    args.extend_from_slice(extra);
    let out = kdiff(&args);
    assert!(
        out.status.success(),
        "install {name} failed:\nstdout={}\nstderr={}",
        stdout(&out),
        String::from_utf8_lossy(&out.stderr)
    );
}

// --------------------------------------------------------------------------
// Fixture builders (deterministic, no external tools). Our schemas don't
// validate checksums, so chunk/header CRC fields are left zero for clean diffs.
// --------------------------------------------------------------------------

fn push_chunk(o: &mut Vec<u8>, typ: &[u8], data: &[u8]) {
    o.extend_from_slice(&(data.len() as u32).to_be_bytes());
    o.extend_from_slice(typ);
    o.extend_from_slice(data);
    o.extend_from_slice(&0u32.to_be_bytes()); // crc (unvalidated)
}

fn png(width: u32, height: u32, color_type: u8, extra_text: Option<&[u8]>) -> Vec<u8> {
    let mut o = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    o.extend_from_slice(&13u32.to_be_bytes()); // ihdr_len
    o.extend_from_slice(b"IHDR");
    o.extend_from_slice(&width.to_be_bytes());
    o.extend_from_slice(&height.to_be_bytes());
    o.extend_from_slice(&[8, color_type, 0, 0, 0]); // depth, color, comp, filter, interlace
    o.extend_from_slice(&0u32.to_be_bytes()); // ihdr_crc
    push_chunk(&mut o, b"IDAT", &[0x78, 0x9c, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01]);
    if let Some(t) = extra_text {
        push_chunk(&mut o, b"tEXt", t);
    }
    push_chunk(&mut o, b"IEND", &[]);
    o
}

fn sqlite_header(page_size: u16, db_pages: u32, change_counter: u32, user_version: u32) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(b"SQLite format 3\0");
    o.extend_from_slice(&page_size.to_be_bytes());
    o.extend_from_slice(&[1, 1, 0, 64, 32, 32]); // write/read ver, reserved, payload fractions
    o.extend_from_slice(&change_counter.to_be_bytes());
    o.extend_from_slice(&db_pages.to_be_bytes());
    o.extend_from_slice(&0u32.to_be_bytes()); // first_freelist_trunk_page
    o.extend_from_slice(&0u32.to_be_bytes()); // total_freelist_pages
    o.extend_from_slice(&0u32.to_be_bytes()); // schema_cookie
    o.extend_from_slice(&4u32.to_be_bytes()); // schema_format_number
    o.extend_from_slice(&0u32.to_be_bytes()); // default_page_cache_size
    o.extend_from_slice(&0u32.to_be_bytes()); // largest_root_btree_page
    o.extend_from_slice(&1u32.to_be_bytes()); // text_encoding = utf8
    o.extend_from_slice(&user_version.to_be_bytes());
    o.extend_from_slice(&0u32.to_be_bytes()); // incremental_vacuum_mode
    o.extend_from_slice(&0u32.to_be_bytes()); // application_id
    o.extend_from_slice(&[0u8; 20]); // reserved
    o.extend_from_slice(&change_counter.to_be_bytes()); // version_valid_for
    o.extend_from_slice(&3045000u32.to_be_bytes()); // sqlite_version_number
    assert_eq!(o.len(), 100);
    o
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

// --------------------------------------------------------------------------
// Tests
// --------------------------------------------------------------------------

#[test]
fn png_scalar_and_enum_diff() {
    ensure_installed("png", "png.ksy", &["--ext", "png", "--magic", "89504e47"]);
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.png", &png(1, 1, 2, None));
    let b = write(dir.path(), "b.png", &png(2, 1, 2, None));

    let out = kdiff(&["diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        stdout(&out),
        "\
~ Png (Png)
~   ihdr (Png_IhdrChunk)
-     width: 1
+     width: 2
"
    );
}

#[test]
fn png_list_insert_diff() {
    ensure_installed("png", "png.ksy", &["--ext", "png", "--magic", "89504e47"]);
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.png", &png(2, 1, 2, None));
    let b = write(dir.path(), "b.png", &png(2, 1, 2, Some(b"Author\x00kdiff")));

    let out = kdiff(&["diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        stdout(&out),
        "\
~ Png (Png)
~   chunks [2 \u{2192} 3]
+     [1]: Png_Chunk
+       len: 12
+       type: \"tEXt\"
+       body: 41 75 74 68 6f 72 00 6b 64 69 66 66
+       crc: 0
"
    );
}

#[test]
fn png_identical_is_empty() {
    ensure_installed("png", "png.ksy", &["--ext", "png", "--magic", "89504e47"]);
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.png", &png(3, 3, 6, None));
    let b = write(dir.path(), "b.png", &png(3, 3, 6, None));
    let out = kdiff(&["diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "");
}

#[test]
fn sqlite_header_diff() {
    ensure_installed("sqlite", "sqlite3.ksy", &["--name", "sqlite", "--ext", "sqlite,db"]);
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.sqlite", &sqlite_header(4096, 2, 2, 1));
    let b = write(dir.path(), "b.sqlite", &sqlite_header(4096, 4, 2, 42));

    let out = kdiff(&["diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        stdout(&out),
        "\
~ Sqlite3 (Sqlite3)
-   database_size_in_pages: 2
+   database_size_in_pages: 4
-   user_version: 1
+   user_version: 42
"
    );
}

#[test]
fn git_commit_stage_diff_roundtrip() {
    ensure_installed("png", "png.ksy", &["--ext", "png", "--magic", "89504e47"]);
    let repo = tempfile::tempdir().unwrap();
    let r = repo.path();

    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(r)
            .env("KDIFF_DATA_DIR", shared_home().join("data"))
            .env("KDIFF_CONFIG_DIR", shared_home().join("config"))
            .env("KDIFF_SUPPORT_DIR", workspace_root())
            .env("KDIFF_NO_PAGER", "1")
            .env("NO_COLOR", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .expect("run git");
        assert!(out.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&out.stderr));
        out
    };

    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "Tester"]);
    let driver = format!("{} git-diff", env!("CARGO_BIN_EXE_kdiff"));
    git(&["config", "diff.kdiff.command", &driver]);
    std::fs::write(r.join(".gitattributes"), "*.png diff=kdiff\n").unwrap();

    std::fs::write(r.join("img.png"), png(1, 1, 2, None)).unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init"]);

    // Stage a semantic change (1x1 -> 2x1) and diff the staged version.
    std::fs::write(r.join("img.png"), png(2, 1, 2, None)).unwrap();
    git(&["add", "img.png"]);

    let out = git(&["-c", "core.pager=cat", "diff", "--cached", "img.png"]);
    assert_eq!(
        stdout(&out),
        "\
kdiff img.png:
~ Png (Png)
~   ihdr (Png_IhdrChunk)
-     width: 1
+     width: 2
"
    );
}

#[test]
fn install_from_http_url() {
    let ksy = std::fs::read(workspace_root().join("schemas/png.ksy")).unwrap();

    // Minimal one-shot HTTP server so we exercise the URL fetch path (curl).
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = ksy.clone();
    let handle = std::thread::spawn(move || {
        if let Ok((mut sock, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = sock.read(&mut buf);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = sock.write_all(header.as_bytes());
            let _ = sock.write_all(&body);
        }
    });

    let url = format!("http://127.0.0.1:{port}/png.ksy");
    let out = kdiff(&["install", &url, "--name", "pngurl"]);
    let _ = handle.join();
    assert!(out.status.success(), "url install failed: {}", String::from_utf8_lossy(&out.stderr));

    let list = kdiff(&["list"]);
    assert!(stdout(&list).contains("pngurl"), "list: {}", stdout(&list));
}
