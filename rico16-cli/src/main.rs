//! `rico16` — the rico-16 fantasy console binary (SPEC §15).
//!
//! Subcommand wiring only; every behaviour lives in a library crate
//! (AGENTS.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use rico16_cart::png::{self, Variant};
use rico16_cart::{BuildOptions, CartMeta, CartToml, FORMAT_V1};

mod error;

use error::CliError;

/// The rico-16 fantasy console.
#[derive(Debug, Parser)]
#[command(name = "rico16", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Scaffold a new project directory.
    New {
        /// Directory to create.
        name: String,
        /// Bake a palette preset into the cart (`rgb332`).
        #[arg(long)]
        palette: Option<String>,
    },
    /// Build a project into a packed PNG.
    Build {
        /// Project directory containing `cart.toml` (default: the current directory).
        #[arg(default_value = ".")]
        project: PathBuf,
        /// Custom cover art PNG.
        #[arg(long)]
        label: Option<PathBuf>,
        /// Force a PNG size variant: `default` or `overflow`.
        #[arg(long)]
        variant: Option<String>,
    },
    /// Run a project directory or a packed PNG.
    Run {
        /// Project directory or `.png` file (default: the current directory).
        #[arg(default_value = ".")]
        cart: PathBuf,
    },
    /// Reconstruct a project from a shared cart PNG.
    Unpack {
        /// The cart PNG.
        png: PathBuf,
        /// Directory to write the project into.
        output_dir: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::New { name, palette } => new_project(&name, palette.as_deref()),
        Command::Build {
            project,
            label,
            variant,
        } => build(&project, label, variant.as_deref()),
        Command::Run { cart } => {
            rico16_player::run(rico16_player::CartSource::from_path(cart))?;
            Ok(())
        }
        Command::Unpack { png, output_dir } => unpack(&png, &output_dir),
    }
}

/// SPEC §15.1 — scaffold a project directory.
fn new_project(name: &str, palette: Option<&str>) -> Result<(), CliError> {
    let root = PathBuf::from(name);
    if root.join("cart.toml").exists() {
        return Err(CliError::Message(format!(
            "{} already contains a cart.toml",
            root.display()
        )));
    }

    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("assets"))?;
    std::fs::create_dir_all(root.join("build"))?;
    std::fs::write(root.join(".gitignore"), "build/\n")?;
    std::fs::write(root.join("src/main.rhai"), STARTER)?;

    let title = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "cart".to_string());

    let mut manifest = CartToml {
        format: FORMAT_V1,
        cart: CartMeta {
            title,
            author: String::new(),
            version: "0.1.0".to_string(),
            entry: None,
            output: None,
            extra: Default::default(),
        },
        sprites: Vec::new(),
        map: None,
        palette: None,
        extra_tables: Default::default(),
        extra_values: Default::default(),
    };

    if let Some(preset) = palette {
        match preset {
            // SPEC §4.11 — bake the preset into the cart as ordinary palette data.
            "rgb332" => {
                let words = rico16_core::palette::rgb332();
                let mut rgba = Vec::with_capacity(256 * 4);
                for word in words {
                    rgba.push(rico16_core::palette::expand5(word & 0x1F));
                    rgba.push(rico16_core::palette::expand5((word >> 5) & 0x1F));
                    rgba.push(rico16_core::palette::expand5((word >> 10) & 0x1F));
                    rgba.push(0xFF);
                }
                let png = png::encode_png(16, 16, &rgba)?;
                std::fs::write(root.join("assets/palette.png"), png)?;
                manifest.palette = Some("assets/palette.png".to_string());
            }
            other => {
                return Err(CliError::Message(format!(
                    "unknown palette preset `{other}`, expected `rgb332`"
                )));
            }
        }
    }

    std::fs::write(root.join("cart.toml"), manifest.to_canonical_string())?;
    println!("created {}", root.display());
    Ok(())
}

/// SPEC §15.2 — build a project into a packed PNG.
fn build(project: &Path, label: Option<PathBuf>, variant: Option<&str>) -> Result<(), CliError> {
    let variant = match variant {
        None => None,
        Some("default") => Some(Variant::DEFAULT),
        Some("overflow") => Some(Variant::OVERFLOW),
        Some(other) => {
            return Err(CliError::Message(format!(
                "unknown variant `{other}`, expected `default` or `overflow`"
            )));
        }
    };

    let options = BuildOptions { label, variant };
    let output = rico16_cart::build_project(project, &options)?;

    let manifest = read_manifest(project)?;
    let output_path = project.join(manifest.output());
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_path, &output.png)?;

    println!(
        "{}: {} of {} payload bytes used (variant {})",
        output_path.display(),
        output.payload_used,
        output.capacity,
        output.variant.id
    );
    Ok(())
}

/// SPEC §15.4 — reconstruct a project from a cart PNG.
fn unpack(cart_png: &Path, output_dir: &Path) -> Result<(), CliError> {
    let bytes = std::fs::read(cart_png)?;
    let files = rico16_cart::unpack_project(&bytes)?;
    for file in &files.files {
        let path = output_dir.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &file.bytes)?;
    }
    println!(
        "unpacked {} files into {}",
        files.files.len(),
        output_dir.display()
    );
    Ok(())
}

fn read_manifest(project: &Path) -> Result<CartToml, CliError> {
    let path = project.join("cart.toml");
    let text = std::fs::read_to_string(&path)?;
    Ok(CartToml::parse_str(&text)?)
}

/// SPEC §15.1 — the starter `src/main.rhai`.
const STARTER: &str = "fn on_init() {\n}\n\nfn on_update(dt) {\n}\n\nfn on_draw() {\n}\n";
