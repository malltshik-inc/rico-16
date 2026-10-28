//! `rico16-player` — run one cart in a window.
//!
//! Usage: `rico16-player <cart.r16>`. `Esc` closes, `F11` toggles fullscreen,
//! `Ctrl+R` restarts the cart. `rico16 run <cart>` is the supported entry
//! point; this binary is a convenience for working on the player itself.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: rico16-player <cart.r16>");
        return ExitCode::from(2);
    };

    match rico16_player::run(Path::new(&path)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
