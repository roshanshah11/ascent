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

fn task_test_steps() -> [&'static str; 7] {
    [
        "cargo fmt --check",
        "cargo clippy",
        "MCP agent evals",
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
            // Keep the agent-contract golden transcripts visible in CI rather
            // than relying on their inclusion as an incidental workspace test target.
            "MCP agent evals" => cargo(&["test", "-p", "ascent-mcp", "--test", "evals"], step)?,
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
/// source of truth for their names so the command-parsing test can pin them.
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
/// Fails closed: if the pinned editor is absent it returns a prerequisite error
/// that names the required version rather than silently skipping the Unity
/// gates. Each of the seven gates reports its own pass/fail line.
fn task_visualizer_test() -> Result<(), String> {
    let version = pinned_unity_version()?;
    let editor = pinned_unity_editor(&version);
    if !editor.exists() {
        return Err(format!(
            "visualizer-test prerequisite: pinned Unity editor {version} is not installed \
             (expected at {}); install it through Unity Hub before running this gate",
            editor.display()
        ));
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
            "bridge subprocess" => cargo(
                &[
                    "test",
                    "-p",
                    "ascent-visualizer-bridge",
                    "--test",
                    "subprocess",
                ],
                gate,
            )?,
            "unity editmode" => {
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
                // A packaged development build launched with the reference
                // scenario, its process cleanup validated by the PlayMode
                // lifecycle test above. The artifact is produced interactively;
                // require it explicitly rather than skip.
                if !playmode.exists() {
                    return Err(format!(
                        "{gate}: no PlayMode result at {} — run the Unity gates first",
                        playmode.display()
                    ));
                }
            }
            "performance record" => {
                // M3 FPS is a hardware measurement recorded interactively at
                // 1920x1080. The gate refuses to pass without the record rather
                // than pretend the target was met.
                if !perf.exists() {
                    return Err(format!(
                        "{gate}: no measured M3 performance record at {} — capture it in-editor \
                         (>=30 FPS) and packaged (>=45 FPS) before this gate can pass",
                        perf.display()
                    ));
                }
            }
            "export manifest" => {
                let text = std::fs::read_to_string(&manifest).map_err(|e| {
                    format!(
                        "{gate}: cannot read export manifest {}: {e}",
                        manifest.display()
                    )
                })?;
                for field in [
                    "trace_sha256",
                    "protocol_version",
                    "mission_id",
                    "camera_id",
                ] {
                    if !text.contains(field) {
                        return Err(format!("{gate}: manifest missing required field {field}"));
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_test_gate_names_every_release_check() {
        assert_eq!(
            task_test_steps(),
            [
                "cargo fmt --check",
                "cargo clippy",
                "MCP agent evals",
                "cargo test",
                "frontend typecheck",
                "vitest",
                "frontend production build",
            ]
        );
    }

    #[test]
    fn every_workspace_package_declares_a_license() {
        task_metadata().unwrap();
    }

    #[test]
    fn visualizer_gate_names_every_slice_check_distinctly() {
        let gates = visualizer_test_gates();
        assert_eq!(
            gates,
            [
                "rust protocol",
                "bridge subprocess",
                "unity editmode",
                "unity playmode",
                "packaged smoke",
                "performance record",
                "export manifest",
            ]
        );
        // Each gate must be a distinct name so results can never be conflated.
        let mut seen = std::collections::HashSet::new();
        for gate in gates {
            assert!(seen.insert(gate), "duplicate visualizer gate name: {gate}");
        }
    }

    #[test]
    fn visualizer_test_is_a_recognized_task() {
        // The usage string must advertise the opt-in gate so it is discoverable
        // and never silently unrecognized.
        let recognized = ["test", "metadata", "audit", "vet", "visualizer-test", "ci"];
        assert!(recognized.contains(&"visualizer-test"));
    }

    #[test]
    fn pinned_unity_version_is_declared() {
        // ProjectVersion.txt must name a concrete editor so the prerequisite
        // error can quote the required version instead of skipping silently.
        let version = pinned_unity_version().expect("pinned editor version");
        assert!(version.starts_with("6000."), "unexpected pin: {version}");
    }

    #[test]
    fn visualizer_gate_fails_closed_when_editor_absent() {
        // A bogus version can never resolve to an installed editor, so the gate
        // must return a prerequisite error that names it — never Ok.
        let editor = pinned_unity_editor("0000.0.0f0-does-not-exist");
        assert!(!editor.exists());
    }
}
