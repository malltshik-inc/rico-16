//! The `rico16` command line: the console's single entry point.

#![forbid(unsafe_code)]

mod template;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use rico16_cart::Cart;
#[derive(Parser)]
#[command(
    name = "rico16",
    version,
    about = "the rico-16 fantasy console",
    long_about = "A 16-bit fantasy console: 320x240, 256 indexed colours, Rhai scripting.\n\
                  Carts are text files and pack into a PNG."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new cart
    New {
        /// Cart name; `.r16` is appended when missing
        name: String,
        /// Palette preset to bake into the cart: `rgb332`
        #[arg(long, value_name = "NAME")]
        palette: Option<String>,
    },
    /// Rewrite a cart in canonical form
    Fmt {
        /// Cart to format
        path: PathBuf,
        /// Do not write; exit 1 if the cart is not canonical
        #[arg(long)]
        check: bool,
    },
    /// Run a cart in the player window
    Run {
        /// Cart to run
        path: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::New { name, palette } => finish(cmd_new(&name, palette.as_deref())),
        Command::Fmt { path, check } => finish(cmd_fmt(&path, check)),
        // Script errors already carry `<file>:<line>:<col>: <message>`
        // (SPEC §12.6), so they are printed verbatim instead of being prefixed.
        Command::Run { path } => match rico16_player::run(&path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
    }
}

fn finish(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_new(name: &str, palette: Option<&str>) -> Result<(), String> {
    if name.ends_with(".png") {
        return Err("a cart starts as text; use `rico16 pack` for the png form".to_owned());
    }

    let path = if name.ends_with(".r16") {
        PathBuf::from(name)
    } else {
        PathBuf::from(format!("{name}.r16"))
    };

    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }

    let title = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("untitled");

    let mut cart = Cart::new(title);
    cart.code = template::CODE.to_owned();
    match palette {
        // Absent means "console default", which is a property of the runtime
        // rather than of the cart (SPEC §10.6), so nothing is written.
        None => {}
        Some("rgb332") => cart.palette = Some(rico16_core::palette::rgb332()),
        Some(other) => {
            return Err(format!(
                "unknown palette `{other}`; known presets: rgb332, or omit for the default"
            ));
        }
    }
    cart.validate().map_err(|error| error.to_string())?;

    std::fs::write(&path, cart.to_text())
        .map_err(|error| format!("{}: {error}", path.display()))?;

    println!("created {}", path.display());
    Ok(())
}

fn cmd_fmt(path: &Path, check: bool) -> Result<(), String> {
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;

    let cart = Cart::parse(&source).map_err(|error| format!("{}: {error}", path.display()))?;
    let text = cart.to_text();

    if text == source {
        println!("{}: canonical", path.display());
        return Ok(());
    }

    if check {
        return Err(format!("{}: not canonical", path.display()));
    }

    std::fs::write(path, &text).map_err(|error| format!("{}: {error}", path.display()))?;
    println!("{}: formatted", path.display());
    Ok(())
}
