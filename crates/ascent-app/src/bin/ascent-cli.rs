//! Thin shell over ascent_app::cli — all logic lives (and is tested) in
//! the library. Exit code 0 with results on stdout; errors to stderr.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["replay", path] => std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {path}: {e}"))
            .and_then(|jsonl| ascent_app::cli::replay_journal(&jsonl)),
        ["run-study", path, study] => std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {path}: {e}"))
            .and_then(|toml| ascent_app::cli::run_study(&toml, study)),
        ["create-review-bundle", path] => create_review_bundle(path),
        ["verify-review-bundle", path] => verify_review_bundle(path),
        _ => {
            eprint!("{}", ascent_app::cli::USAGE);
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn create_review_bundle(path: &str) -> Result<String, String> {
    let document = ascent_app::Document::default();
    let bundle = ascent_app::MissionReviewBundle::build(
        &document,
        ascent_app::ReviewBundleInput {
            journal: document.journal_jsonl(),
            alignment: None,
            reconciliation: None,
            story: vec![],
            qualifications: vec![
                "golden release smoke fixture; deterministic prediction only".into(),
            ],
        },
    )?;
    let bytes = bundle.canonical_bytes()?;
    std::fs::write(path, &bytes).map_err(|error| format!("cannot write {path}: {error}"))?;
    Ok(format!(
        "wrote {} byte review bundle to {path}",
        bytes.len()
    ))
}

fn verify_review_bundle(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    let reopened = ascent_app::MissionReviewBundle::reopen(&bytes)?;
    Ok(format!(
        "verified review bundle v{} document {}",
        reopened.manifest.bundle_version, reopened.manifest.document_sha256
    ))
}
