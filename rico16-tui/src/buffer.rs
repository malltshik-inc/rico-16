//! SPEC §16.2, §16.5 — a simple text buffer with a cursor.
//!
//! Saving writes the buffer to disk and never throws edits away: a cart that
//! does not parse is a normal state, reported in the console pane.

use std::path::{Path, PathBuf};

/// An editable text buffer.
#[derive(Debug, Clone, Default)]
pub struct TextBuffer {
    lines: Vec<Vec<char>>,
    row: usize,
    col: usize,
    dirty: bool,
    path: Option<PathBuf>,
}

impl TextBuffer {
    /// A buffer from file contents.
    pub fn from_text(text: &str, path: impl Into<Option<PathBuf>>) -> Self {
        let mut lines: Vec<Vec<char>> = text
            .split('\n')
            .map(|line| line.trim_end_matches('\r').chars().collect())
            .collect();
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        Self {
            lines,
            row: 0,
            col: 0,
            dirty: false,
            path: path.into(),
        }
    }

    /// Load a file into a buffer.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(Self::from_text(&text, Some(path.to_path_buf())))
    }

    /// The buffer's file, if it has one.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Whether there are unsaved edits.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The cursor as `(row, col)`.
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    /// Number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// One line, without its newline.
    pub fn line(&self, index: usize) -> String {
        self.lines
            .get(index)
            .map(|l| l.iter().collect())
            .unwrap_or_default()
    }

    /// The whole buffer as text, with a trailing newline per line.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.extend(line.iter());
        }
        out
    }

    /// Insert one character at the cursor.
    pub fn insert_char(&mut self, ch: char) {
        self.lines[self.row].insert(self.col, ch);
        self.col += 1;
        self.dirty = true;
    }

    /// Split the line at the cursor.
    pub fn insert_newline(&mut self) {
        let tail = self.lines[self.row].split_off(self.col);
        self.lines.insert(self.row + 1, tail);
        self.row += 1;
        self.col = 0;
        self.dirty = true;
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.lines[self.row].remove(self.col);
            self.dirty = true;
        } else if self.row > 0 {
            let current = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].len();
            self.lines[self.row].extend(current);
            self.dirty = true;
        }
    }

    /// Delete the character under the cursor.
    pub fn delete(&mut self) {
        if self.col < self.lines[self.row].len() {
            self.lines[self.row].remove(self.col);
            self.dirty = true;
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].extend(next);
            self.dirty = true;
        }
    }

    /// Move the cursor left.
    pub fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].len();
        }
    }

    /// Move the cursor right.
    pub fn move_right(&mut self) {
        if self.col < self.lines[self.row].len() {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    /// Move the cursor up.
    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.lines[self.row].len());
        }
    }

    /// Move the cursor down.
    pub fn move_down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = self.col.min(self.lines[self.row].len());
        }
    }

    /// Move to the start of the line.
    pub fn move_home(&mut self) {
        self.col = 0;
    }

    /// Move to the end of the line.
    pub fn move_end(&mut self) {
        self.col = self.lines[self.row].len();
    }

    /// SPEC §16.5 — write the buffer to disk.
    pub fn save(&mut self) -> std::io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, self.to_text())?;
        self.dirty = false;
        Ok(())
    }
}
