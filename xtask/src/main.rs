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
        "ci" => task_test()
            .and_then(|()| task_audit())
            .and_then(|()| task_vet()),
        _ => Err("usage: cargo xtask <test|metadata|audit|vet|ci>".into()),
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
}
