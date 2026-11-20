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
//!
//! Ninja may decide to re-run the cmake configure (e.g. after a dependency
//! graph change makes CMakeCache newer than build.ninja). The generated
//! `out/CMakeLists.txt` reads its inputs from `$ENV{...}` (IDF_PATH,
//! IDF_TARGET, EXTRA_COMPONENT_DIRS, SDKCONFIG, SDKCONFIG_DEFAULTS,
//! PROJECT_DIR), which the original esp-idf-sys build script had set — so
//! this driver forwards them again; without them the reconfigure dies with
//! "Unknown CMake command idf_build_get_property" and wedges every build.

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
            if let Err(err) = run_ninja(&ninja, &tree) {
                println!(
                    "cargo:warning=incremental C build failed ({err}); C changes need cargo clean -p esp-idf-sys"
                );
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

/// Drive ninja against the SDK tree, forwarding the configure environment
/// so a cmake re-run stays valid; a failed build degrades to a warning
/// instead of panicking (the legacy clean path remains the recovery).
fn run_ninja(ninja: &Path, tree: &Path) -> Result<(), String> {
    let out_dir = tree.ancestors().nth(1).unwrap_or(tree);
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let mut command = Command::new(ninja);
    command.arg("-C").arg(tree);
    for (key, value) in configure_env(&out_dir, &manifest) {
        command.env(key, value);
    }
    let status = command
        .status()
        .map_err(|err| format!("cannot run ninja: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("exit status {status}"))
    }
}

/// The `$ENV{...}` contract of embuild's generated `out/CMakeLists.txt`.
/// Paths go in cmake-friendly forward-slash form.
fn configure_env(out_dir: &Path, manifest: &Path) -> Vec<(&'static str, String)> {
    let fwd = |path: &Path| path.to_string_lossy().replace('\\', "/");
    let mut env = Vec::new();
    if let Some(idf_path) = embuild::espidf::sysenv::idf_path() {
        env.push(("IDF_PATH", idf_path.replace('\\', "/")));
    }
    if let Some(target) = idf_target(out_dir) {
        env.push(("IDF_TARGET", target));
    }
    let components = manifest.join("components");
    if components.is_dir() {
        env.push(("EXTRA_COMPONENT_DIRS", fwd(&components)));
    }
    let sdkconfig = out_dir.join("sdkconfig");
    if sdkconfig.is_file() {
        env.push(("SDKCONFIG", fwd(&sdkconfig)));
    }
    let defaults = manifest.join("sdkconfig.defaults");
    if defaults.is_file() {
        env.push(("SDKCONFIG_DEFAULTS", fwd(&defaults)));
    }
    env.push(("PROJECT_DIR", fwd(manifest)));
    env
}

/// MCU from the [env] table; falls back to CONFIG_IDF_TARGET in the tree's
/// generated sdkconfig.
fn idf_target(out_dir: &Path) -> Option<String> {
    if let Ok(mcu) = std::env::var("MCU") {
        return Some(mcu);
    }
    let sdkconfig = std::fs::read_to_string(out_dir.join("sdkconfig")).ok()?;
    sdkconfig
        .lines()
        .find_map(|line| line.strip_prefix("CONFIG_IDF_TARGET=\""))
        .map(|rest| rest.trim_end_matches('"').to_string())
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
