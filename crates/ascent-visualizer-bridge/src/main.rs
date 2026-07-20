//! `ascent-visualizer-bridge --stdio`: a read-only supervised bridge that
//! streams a deterministic reference-mission trace over stdin/stdout using
//! visualizer protocol v1.
//!
//! There is no network, no file argument, and no command surface. The only
//! accepted transport is `--stdio`.

mod runtime;

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut stdio = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--stdio" => stdio = true,
            other => {
                eprintln!("ascent-visualizer-bridge: unexpected argument {other:?}");
                return ExitCode::from(2);
            }
        }
    }
    if !stdio {
        eprintln!("ascent-visualizer-bridge: --stdio is required");
        return ExitCode::from(2);
    }

    match runtime::serve(io::stdin(), io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ascent-visualizer-bridge: {error}");
            ExitCode::FAILURE
        }
    }
}
