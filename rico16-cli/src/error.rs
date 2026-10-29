//! CLI errors.

/// Anything the `rico16` binary can fail with.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// A plain message.
    #[error("{0}")]
    Message(String),

    /// The cart format rejected something.
    #[error("{0}")]
    Cart(#[from] rico16_cart::CartError),

    /// The player failed.
    #[error("{0}")]
    Player(#[from] rico16_player::PlayerError),

    /// The editor failed.
    #[error("{0}")]
    Tui(#[from] rico16_tui::TuiError),

    /// A file operation failed.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}
