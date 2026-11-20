//! Root build script.
//!
//! 1. Forwards the ESP-IDF sysenv (pins/bindings entry for esp-idf-sys).
//! 2. Drives an **incremental ninja build** of the C components whenever
//!    `components/` changes, so C edits reach the firmware with one
//!    `cargo build` instead of a ~3 min `cargo clean -p esp-idf-sys`.
//!
//! Why this is needed: esp-idf-sys's build script only declares rerun-if
//! inputs for `partitions.csv`, `sdkconfig.defaults` and the bindings header
//! — the C sources under `components/` are not watched, so ninja (which only
//! that build script can drive) never sees C edits. This script bridges the
//! gap: it watches every file under `components/` itself, on rerun it invokes
//! ninja directly against the SDK build tree (which recompiles only the
//! changed `.c` files; the tree persists, so no cmake configure or bindgen
//! reruns), and it emits `TG_C_STAMP` (a hash of watched mtimes) as a
//! rustc-env: when C sources change the stamp changes, which recompiles and
//! **relinks** the root crate — ldproxy reads the `.a` files straight out of
//! the ninja tree, so the fresh C libraries land in the firmware.
//! If the tree or ninja cannot be found (e.g. a fresh checkout before the
//! first esp-idf-sys build), the step degrades with a warning and the
//! legacy `cargo clean -p esp-idf-sys && cargo build` path still works.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

fn main() {
    embuild::espidf::sysenv::output();
    let stamp = watch_components();
    println!("cargo:rustc-env=TG_C_STAMP={stamp}");
}

/// Watch every file under `components/`, run ninja incrementally when the
/// tree exists, and return a stamp that changes only when C inputs change.
fn watch_components() -> u64 {
    let root = Path::new("components");
    if !root.is_dir() {
        return 0;
    }
    let mut files: Vec<PathBuf> = Vec::new();
    collect_files(root, &mut files);

    let mut stamp = 0u64;
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        if let Some(secs) = mtime_secs(file) {
            stamp = stamp.wrapping_mul(31).wrapping_add(secs);
        }
    }

    match (find_sdk_build_tree(), find_ninja()) {
        (Some(tree), Some(ninja)) => {
            let status = Command::new(&ninja).arg("-C").arg(&tree).status();
            match status {
                Ok(status) if status.success() => {}
                Ok(status) => panic!("incremental ninja build of components/ failed: {status}"),
                Err(err) => println!(
                    "cargo:warning=cannot run ninja ({}); C changes need cargo clean -p esp-idf-sys",
                    err
                ),
            }
        }
        (tree, ninja) => {
            if tree.is_none() {
                // First build in a fresh target dir: esp-idf-sys creates the
                // tree itself on its own first run; nothing to drive here.
                println!("cargo:warning=SDK build tree not found; skipping incremental C build");
            }
            if ninja.is_none() {
                println!(
                    "cargo:warning=ninja.exe not found under ~/.espressif/tools or PATH; \
                     C changes need cargo clean -p esp-idf-sys"
                );
            }
        }
    }
    stamp
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            println!("cargo:warning=cannot read {}: {err}", dir.display());
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn mtime_secs(path: &Path) -> Option<u64> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// The SDK ninja tree lives under the esp-idf-sys build output directory,
/// e.g. target/<target>/<profile>/build/esp-idf-sys-*/out/build.
/// Locate it relative to our own OUT_DIR so CARGO_TARGET_DIR stays honored.
fn find_sdk_build_tree() -> Option<PathBuf> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").ok()?);
    // out -> tiny-gate-<hash> -> build -> <profile>
    let build_root = out_dir.ancestors().nth(2)?;
    for entry in std::fs::read_dir(build_root).ok()? {
        let entry = entry.ok()?;
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("esp-idf-sys-")
        {
            continue;
        }
        let tree = entry.path().join("out").join("build");
        if tree.join("build.ninja").is_file() {
            return Some(tree);
        }
    }
    None
}

/// ninja ships with the espressif tools (~/.espressif/tools/ninja/<ver>/).
fn find_ninja() -> Option<PathBuf> {
    for var in ["USERPROFILE", "HOME"] {
        if let Ok(home) = std::env::var(var) {
            let base = Path::new(&home)
                .join(".espressif")
                .join("tools")
                .join("ninja");
            if let Ok(versions) = std::fs::read_dir(&base) {
                for entry in versions.flatten() {
                    let candidate = entry.path().join("ninja.exe");
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                    let candidate = entry.path().join("ninja");
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    // Fall back to PATH resolution by returning a bare command name.
    Some(PathBuf::from("ninja"))
}
