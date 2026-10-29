//! `rico16-player` — run a cart project or a packed PNG without the CLI.

use std::process::ExitCode;

use rico16_player::{CartSource, run};

fn main() -> ExitCode {
    let Some(argument) = std::env::args_os().nth(1) else {
        eprintln!("usage: rico16-player <project-dir|cart.png>");
        return ExitCode::from(2);
    };
    let source = CartSource::from_path(argument);
    match run(source) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
