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
