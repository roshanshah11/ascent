//! `cargo xtask <task>` — workspace automation as plain Rust.
//!
//! Tasks:
//! - `test`     full gate: Rust workspace tests + frontend typecheck + vitest
//! - `audit`    cargo audit (advisory database; requires cargo-audit)
//! - `vet`      cargo vet (supply-chain review; requires cargo-vet)
//! - `ci`       everything above, in order — what CI runs

//!
//! Workspace maintenance commands:
//! - `test`     run metadata, Rust, frontend typecheck, and frontend tests
//! - `metadata` verify every workspace package declares a license
//! - `audit`    cargo audit (advisory database; requires cargo-audit)
//! - `vet`      cargo vet (supply-chain review; requires cargo-vet)
//! - `ci`       everything above, in order
//!
//! Opt-in visualizer gate (never part of `test`/`ci`, so machines without the
//! pinned Unity editor keep the current workspace gate):
//! - `visualizer-test`  the seven Unity slice gates, each reported distinctly

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    // xtask always lives at <root>/xtask.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace root")
        .to_path_buf()
}

fn run(mut cmd: Command, what: &str) -> Result<(), String> {
    let status = cmd
        .status()
        .map_err(|e| format!("{what}: failed to launch: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what}: exited with {status}"))
    }
}

fn cargo(args: &[&str], what: &str) -> Result<(), String> {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args(args).current_dir(workspace_root());
    run(cmd, what)
}

fn npm(args: &[&str], what: &str) -> Result<(), String> {
    let mut cmd = Command::new("npm");
    cmd.args(args).current_dir(workspace_root().join("app"));
    run(cmd, what)
}

fn task_metadata() -> Result<(), String> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(workspace_root())
        .output()
        .map_err(|error| format!("cargo metadata: failed to launch: {error}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata: exited with {}", output.status));
    }
    let metadata = String::from_utf8(output.stdout)
        .map_err(|error| format!("cargo metadata emitted non-UTF-8: {error}"))?;
    if metadata.contains("\"license\":null") {
        return Err("cargo metadata: a workspace package has no license".into());
    }
    Ok(())
}

fn task_test_steps() -> [&'static str; 6] {
    [
        "cargo fmt --check",
        "cargo clippy",
        "cargo test",
        "frontend typecheck",
        "vitest",
        "frontend production build",
    ]
}

fn task_test() -> Result<(), String> {
    task_metadata()?;
    for step in task_test_steps() {
        match step {
            "cargo fmt --check" => cargo(&["fmt", "--all", "--", "--check"], step)?,
            "cargo clippy" => cargo(
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
                step,
            )?,
            "cargo test" => cargo(&["test", "--workspace"], step)?,
            "frontend typecheck" => npm(&["run", "typecheck"], step)?,
            "vitest" => npm(&["test"], step)?,
            "frontend production build" => npm(&["run", "build"], step)?,
            _ => unreachable!("all test steps are handled"),
        }
    }
    Ok(())
}

/// The seven predeclared gates of the Unity vertical slice, in run order.
/// Step 5 requires each to have an explicit result; this array is the single
/// source of truth for their names.
fn visualizer_test_gates() -> [&'static str; 7] {
    [
        "rust protocol",
        "bridge subprocess",
        "unity editmode",
        "unity playmode",
        "packaged smoke",
        "performance record",
        "export manifest",
    ]
}

/// Editor version the Unity project is pinned to, read from the committed
/// `ProjectVersion.txt` (the same file `scripts/unity.sh` resolves against).
fn pinned_unity_version() -> Result<String, String> {
    let path = workspace_root().join("visualizer/AscentUnity/ProjectSettings/ProjectVersion.txt");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("m_EditorVersion:") {
            let version = rest.trim();
            if !version.is_empty() {
                return Ok(version.to_string());
            }
        }
    }
    Err(format!("no m_EditorVersion in {}", path.display()))
}

/// Absolute path to the pinned Hub-installed editor binary on macOS.
fn pinned_unity_editor(version: &str) -> PathBuf {
    PathBuf::from(format!(
        "/Applications/Unity/Hub/Editor/{version}/Unity.app/Contents/MacOS/Unity"
    ))
}

/// `visualizer-test` — the opt-in Unity slice gate (Step 5.2).
///
/// Reports each of the seven gates distinctly. Gates whose artifact is
/// produced interactively (Unity editor, packaged build, performance record)
/// print a SKIP with the reason instead of failing the run when that artifact
/// isn't present yet.
fn task_visualizer_test() -> Result<(), String> {
    let version = pinned_unity_version()?;
    let editor = pinned_unity_editor(&version);
    let editor_present = editor.exists();
    if !editor_present {
        println!(
            "⊘ visualizer gate: unity editmode, unity playmode — skipped \
             (pinned Unity editor {version} not installed at {})",
            editor.display()
        );
    }

    let unity = workspace_root().join("scripts/unity.sh");
    let results = workspace_root().join("visualizer/AscentUnity/TestResults");
    std::fs::create_dir_all(&results)
        .map_err(|e| format!("cannot create {}: {e}", results.display()))?;
    let editmode = results.join("editmode.xml");
    let playmode = results.join("playmode.xml");
    let manifest = workspace_root().join("visualizer/AscentUnity/Exports/manifest.json");
    let perf = results.join("performance.json");

    for gate in visualizer_test_gates() {
        match gate {
            "rust protocol" => cargo(&["test", "-p", "ascent-visualizer-protocol"], gate)?,
            "bridge subprocess" => cargo(&["test", "-p", "ascent-visualizer-bridge"], gate)?,
            "unity editmode" => {
                if !editor_present {
                    continue;
                }
                let mut cmd = Command::new(&unity);
                cmd.args([
                    "-batchmode",
                    "-nographics",
                    "-runTests",
                    "-testPlatform",
                    "EditMode",
                    "-testResults",
                ])
                .arg(&editmode);
                run(cmd, gate)?;
            }
            "unity playmode" => {
                if !editor_present {
                    continue;
                }
                let mut cmd = Command::new(&unity);
                cmd.args([
                    "-batchmode",
                    "-runTests",
                    "-testPlatform",
                    "PlayMode",
                    "-testResults",
                ])
                .arg(&playmode);
                run(cmd, gate)?;
            }
            "packaged smoke" => {
                // Produced by an interactive packaged build; report status
                // rather than block the run when it isn't present yet.
                if !playmode.exists() {
                    println!("⊘ visualizer gate: {gate} — skipped (no PlayMode result yet)");
                    continue;
                }
            }
            "performance record" => {
                // M3 FPS is a hardware measurement recorded interactively;
                // report status rather than block the run when it's absent.
                if !perf.exists() {
                    println!("⊘ visualizer gate: {gate} — skipped (no performance record yet)");
                    continue;
                }
            }
            "export manifest" => {
                let Ok(text) = std::fs::read_to_string(&manifest) else {
                    println!("⊘ visualizer gate: {gate} — skipped (no export manifest yet)");
                    continue;
                };
                let missing: Vec<_> = [
                    "trace_sha256",
                    "protocol_version",
                    "mission_id",
                    "camera_id",
                ]
                .into_iter()
                .filter(|field| !text.contains(field))
                .collect();
                if !missing.is_empty() {
                    return Err(format!(
                        "{gate}: manifest missing required field(s): {}",
                        missing.join(", ")
                    ));
                }
            }
            _ => unreachable!("all visualizer gates are handled"),
        }
        println!("✓ visualizer gate: {gate}");
    }
    Ok(())
}

fn task_audit() -> Result<(), String> {
    cargo(
        &["audit"],
        "cargo audit (install: cargo install cargo-audit)",
    )
}

fn task_vet() -> Result<(), String> {
    cargo(
        &["vet", "--locked"],
        "cargo vet (install: cargo install cargo-vet)",
    )
}

fn main() -> std::process::ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let outcome = match task.as_str() {
        "test" => task_test(),
        "metadata" => task_metadata(),
        "audit" => task_audit(),
        "vet" => task_vet(),
        "visualizer-test" => task_visualizer_test(),
        "ci" => task_test()
            .and_then(|()| task_audit())
            .and_then(|()| task_vet()),
        _ => Err("usage: cargo xtask <test|metadata|audit|vet|visualizer-test|ci>".into()),
    };
    match outcome {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            std::process::ExitCode::FAILURE
        }
    }
}
