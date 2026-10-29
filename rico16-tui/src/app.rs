//! SPEC §16 — the ratatui editor.

use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use rico16_core::palette::{Palette, expand5};

use crate::TuiError;
use crate::buffer::TextBuffer;
use crate::game::{self, EmbeddedGame, Tick};
use crate::highlight::{Token, highlight};
use crate::tree::{Entry, project_files};

/// The keys shown in the footer, so the editor says how to drive it.
pub const HOTKEYS: &[(&str, &str)] = &[
    ("F1", "help"),
    ("F5", "run"),
    ("F6", "stop"),
    ("F7", "restart"),
    ("F8", "window"),
    ("^S", "save"),
    ("Tab", "pane"),
    ("^Q", "quit"),
];

/// The full key list shown by the help modal.
pub const HELP: &[(&str, &str)] = &[
    ("F1 or ?", "show or hide this help"),
    ("F5", "build and run the cart in the embedded preview"),
    ("F6", "stop the running cart"),
    ("F7", "restart the running cart"),
    ("F8", "run the cart in its own native window"),
    (
        "Ctrl+S",
        "save the current file; hot-reloads a running cart",
    ),
    ("Tab", "move focus between the tree, editor and game panes"),
    ("Esc", "leave the game pane, or close this help"),
    ("Ctrl+Q", "quit the editor"),
    (
        "Up / Down",
        "move in the file tree, or the cursor in the editor",
    ),
    ("Left / Right", "move the cursor"),
    ("Home / End", "start or end of line"),
    ("Enter", "open the selected file, or start a new line"),
    ("Backspace", "delete before the cursor"),
    (
        "Arrows, Z X C V",
        "play the cart while the game pane is focused",
    ),
];

/// How to start the editor.
#[derive(Debug, Clone)]
pub struct EditorOptions {
    /// The project root.
    pub project: PathBuf,
    /// The `rico16` binary to spawn for the windowed run.
    pub program: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Tree,
    Editor,
    Game,
}

struct Editor {
    project: PathBuf,
    program: PathBuf,
    files: Vec<Entry>,
    selected: usize,
    buffer: TextBuffer,
    focus: Focus,
    console: Vec<String>,
    child: Option<Child>,
    game: Option<EmbeddedGame>,
    help: bool,
    output: Receiver<String>,
    sender: Sender<String>,
    palette: Palette,
    should_quit: bool,
}

impl Editor {
    fn new(options: EditorOptions) -> Self {
        let (sender, output) = mpsc::channel();
        let mut editor = Self {
            files: project_files(&options.project),
            project: options.project,
            program: options.program,
            selected: 0,
            buffer: TextBuffer::default(),
            focus: Focus::Editor,
            console: Vec::new(),
            child: None,
            game: None,
            help: false,
            output,
            sender,
            palette: Palette::new(),
            should_quit: false,
        };
        editor.reload_palette();
        editor.open_first_source();
        editor
    }

    /// SPEC §16.2 — load the cart's palette for the inspector.
    fn reload_palette(&mut self) {
        if let Ok(cart) = rico16_cart::assemble_project(&self.project) {
            self.palette = Palette::from_entries(cart.palette);
        } else {
            self.palette = Palette::new();
        }
    }

    fn open_first_source(&mut self) {
        let target = self
            .files
            .iter()
            .position(|f| f.name.ends_with(".rhai"))
            .or_else(|| self.files.iter().position(|f| !f.is_dir));
        if let Some(index) = target {
            self.selected = index;
            self.open_selected();
        }
    }

    fn refresh_files(&mut self) {
        self.files = project_files(&self.project);
        if self.selected >= self.files.len() {
            self.selected = self.files.len().saturating_sub(1);
        }
    }

    fn open_selected(&mut self) {
        let Some(entry) = self.files.get(self.selected) else {
            return;
        };
        if entry.is_dir {
            return;
        }
        match TextBuffer::open(&entry.path) {
            Ok(buffer) => {
                self.buffer = buffer;
                self.focus = Focus::Editor;
            }
            Err(e) => self.log(format!("cannot open {}: {e}", entry.name)),
        }
    }

    /// SPEC §16.3, §16.5 — save, then hot-reload by restarting the cart.
    fn save(&mut self) {
        let running_embedded = self.game.is_some();
        let running_window = self.child.is_some();

        match self.buffer.save() {
            Ok(()) => {
                self.log(format!(
                    "saved {}",
                    self.buffer
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                ));
                self.reload_palette();
                self.refresh_files();
                if running_embedded {
                    self.run_embedded();
                } else if running_window {
                    self.run_windowed();
                }
            }
            Err(e) => self.log(format!("save failed: {e}")),
        }
    }

    /// SPEC §16.3, §16.4 — build and run inside the editor.
    fn run_embedded(&mut self) {
        self.stop_cart();
        match EmbeddedGame::start(&self.project) {
            Ok(game) => {
                self.game = Some(game);
                self.focus = Focus::Game;
                self.log("running in the embedded preview");
            }
            Err(e) => self.log(format!("build failed: {e}")),
        }
    }

    /// SPEC §16.3, §16.4 — run in the cart's own native window.
    fn run_windowed(&mut self) {
        self.stop_cart();
        self.game = None;
        let mut command = Command::new(&self.program);
        command
            .arg("run")
            .arg(&self.project)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match command.spawn() {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    forward(stdout, self.sender.clone());
                }
                if let Some(stderr) = child.stderr.take() {
                    forward(stderr, self.sender.clone());
                }
                self.child = Some(child);
                self.log("running in a window");
            }
            Err(e) => self.log(format!("cannot run: {e}")),
        }
    }

    fn stop_cart(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
            self.log("stopped the window");
        }
    }

    fn stop_game(&mut self) {
        if self.game.take().is_some() {
            self.log("stopped the preview");
        }
        self.stop_cart();
        if self.focus == Focus::Game {
            self.focus = Focus::Editor;
        }
    }

    fn restart_game(&mut self) {
        if self.game.is_some() {
            self.run_embedded();
        } else if self.child.is_some() {
            self.run_windowed();
        } else {
            self.run_embedded();
        }
    }

    /// SPEC §16.4 — advance the embedded cart and collect what it printed.
    fn tick_game(&mut self) {
        let (tick, output) = {
            let Some(game) = self.game.as_mut() else {
                return;
            };
            (game.tick(), game.take_output())
        };

        let mut lines = output;
        let mut stop = false;
        match tick {
            Tick::Running => {}
            Tick::Quit(message) => {
                if let Some(message) = message {
                    lines.push(message);
                }
                lines.push("the cart quit".to_string());
                stop = true;
            }
            Tick::Error(error) => {
                lines.push(error);
                stop = true;
            }
        }

        for line in lines {
            self.log(line);
        }
        if stop {
            self.game = None;
            self.focus = Focus::Editor;
        }
    }

    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Tree => Focus::Editor,
            Focus::Editor => {
                if self.game.is_some() {
                    Focus::Game
                } else {
                    Focus::Tree
                }
            }
            Focus::Game => Focus::Tree,
        };
    }

    fn log(&mut self, line: impl Into<String>) {
        self.console.push(line.into());
        let overflow = self.console.len().saturating_sub(500);
        if overflow > 0 {
            self.console.drain(0..overflow);
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let pressed = key.kind != KeyEventKind::Release;

        if self.help {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('?') | KeyCode::Enter
            ) {
                self.help = false;
            }
            return;
        }

        if pressed {
            match key.code {
                KeyCode::F(1) => {
                    self.help = true;
                    return;
                }
                KeyCode::Char('?') => {
                    self.help = true;
                    return;
                }
                KeyCode::Char('q') if ctrl => {
                    self.should_quit = true;
                    return;
                }
                KeyCode::Char('s') if ctrl => {
                    self.save();
                    return;
                }
                KeyCode::F(5) => {
                    self.run_embedded();
                    return;
                }
                KeyCode::F(6) => {
                    self.stop_game();
                    return;
                }
                KeyCode::F(7) => {
                    self.restart_game();
                    return;
                }
                KeyCode::F(8) => {
                    self.run_windowed();
                    return;
                }
                KeyCode::Tab => {
                    self.cycle_focus();
                    return;
                }
                KeyCode::Esc => {
                    if self.focus == Focus::Game {
                        self.focus = Focus::Editor;
                    }
                    return;
                }
                _ => {}
            }
        }

        // SPEC §16.4 — while the game pane is focused the cart owns the keys.
        if self.focus == Focus::Game {
            if let (Some(game), Some(button)) = (self.game.as_mut(), game::button_for_key(key.code))
            {
                game.set_button(button, pressed);
            }
            return;
        }

        if !pressed {
            return;
        }

        match key.code {
            KeyCode::Char(ch) if !ctrl && self.focus == Focus::Editor => {
                self.buffer.insert_char(ch)
            }
            KeyCode::Backspace if self.focus == Focus::Editor => self.buffer.backspace(),
            KeyCode::Delete if self.focus == Focus::Editor => self.buffer.delete(),
            KeyCode::Enter if self.focus == Focus::Editor => self.buffer.insert_newline(),
            KeyCode::Left => self.buffer.move_left(),
            KeyCode::Right => self.buffer.move_right(),
            KeyCode::Home => self.buffer.move_home(),
            KeyCode::End => self.buffer.move_end(),
            KeyCode::Up => match self.focus {
                Focus::Tree => self.selected = self.selected.saturating_sub(1),
                _ => self.buffer.move_up(),
            },
            KeyCode::Down => match self.focus {
                Focus::Tree => {
                    if self.selected + 1 < self.files.len() {
                        self.selected += 1;
                    }
                }
                _ => self.buffer.move_down(),
            },
            KeyCode::Enter => self.open_selected(),
            _ => {}
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let game_rows = if self.game.is_some() {
            (area.height / 3).clamp(12, 40)
        } else {
            0
        };
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(game_rows),
                Constraint::Length(8),
                Constraint::Length(1),
            ])
            .split(area);

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(30),
                Constraint::Min(20),
                Constraint::Length(34),
            ])
            .split(rows[0]);

        self.draw_tree(frame, columns[0]);
        self.draw_editor(frame, columns[1]);
        self.draw_palette(frame, columns[2]);
        if let Some(game) = &self.game {
            self.draw_game(frame, rows[1], game);
        }
        self.draw_console(frame, rows[2]);
        self.draw_footer(frame, rows[3]);
        if self.help {
            self.draw_help(frame, area);
        }
    }

    fn draw_tree(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .files
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let indent = "  ".repeat(entry.depth);
                let marker = if entry.is_dir { "▸ " } else { "  " };
                let text = format!("{indent}{marker}{}", entry.name);
                let style = if i == self.selected {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else if entry.is_dir {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(text)).style(style)
            })
            .collect();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" project ")
            .border_style(self.focus_style(Focus::Tree));
        frame.render_widget(List::new(items).block(block), area);
    }

    fn draw_editor(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .buffer
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "no file".to_string());
        let dirty = if self.buffer.is_dirty() { " *" } else { "" };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title}{dirty} "))
            .border_style(self.focus_style(Focus::Editor));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let lines: Vec<Line> = (0..self.buffer.line_count())
            .map(|row| Line::from(styled(&self.buffer.line(row))))
            .collect();
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);

        if self.focus == Focus::Editor {
            let (row, col) = self.buffer.cursor();
            let x = inner.x + col as u16;
            let y = inner.y + row as u16;
            if x < inner.right() && y < inner.bottom() {
                frame.set_cursor_position((x, y));
            }
        }
    }

    fn draw_game(&self, frame: &mut Frame, area: Rect, game: &EmbeddedGame) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" game (preview) ")
            .border_style(self.focus_style(Focus::Game));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        game::render_frame(game.rgba(), inner, frame.buffer_mut());
    }

    fn draw_palette(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(" palette ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines = Vec::new();
        for row in 0..16u16 {
            let mut spans = Vec::new();
            for col in 0..16u16 {
                let index = (row * 16 + col) as usize;
                let word = self.palette.word(index);
                let r = expand5(word & 0x1F);
                let g = expand5((word >> 5) & 0x1F);
                let b = expand5((word >> 10) & 0x1F);
                spans.push(Span::styled("  ", Style::default().bg(Color::Rgb(r, g, b))));
            }
            lines.push(Line::from(spans));
        }
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn draw_console(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(" console ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let height = inner.height as usize;
        let start = self.console.len().saturating_sub(height);
        let lines: Vec<Line> = self.console[start..]
            .iter()
            .map(|l| Line::from(l.clone()))
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let (state, state_bg) = if self.game.is_some() {
            ("running", Color::Green)
        } else if self.child.is_some() {
            ("window", Color::Cyan)
        } else {
            ("stopped", Color::DarkGray)
        };

        let mut spans = vec![Span::styled(
            format!(" {state} "),
            Style::default().fg(Color::Black).bg(state_bg),
        )];
        for (key, what) in HOTKEYS {
            spans.push(Span::styled(
                format!(" {key}"),
                Style::default().fg(Color::Yellow),
            ));
            spans.push(Span::styled(
                format!(" {what} "),
                Style::default().fg(Color::Gray),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn draw_help(&self, frame: &mut Frame, area: Rect) {
        let width = 64.min(area.width);
        let height = (HELP.len() as u16 + 2).min(area.height);
        let popup = Rect::new(
            area.x + (area.width.saturating_sub(width)) / 2,
            area.y + (area.height.saturating_sub(height)) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup);

        let lines: Vec<Line> = HELP
            .iter()
            .map(|(key, what)| {
                Line::from(vec![
                    Span::styled(format!("  {key:<15}"), Style::default().fg(Color::Yellow)),
                    Span::raw(what.to_string()),
                ])
            })
            .collect();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" keys — F1 or Esc to close ");
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .style(Style::default().bg(Color::Black))
                .wrap(Wrap { trim: false }),
            popup,
        );
    }

    fn focus_style(&self, focus: Focus) -> Style {
        if self.focus == focus {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    }
}

/// SPEC §16 — run the editor until the user quits.
pub fn run(options: EditorOptions) -> Result<(), TuiError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut editor = Editor::new(options);
    let result = event_loop(&mut terminal, &mut editor);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    editor.stop_game();
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    editor: &mut Editor,
) -> Result<(), TuiError> {
    while !editor.should_quit {
        editor.tick_game();
        while let Ok(line) = editor.output.try_recv() {
            editor.log(line);
        }

        terminal.draw(|frame| editor.draw(frame))?;

        // SPEC §14.2 — a running preview needs to be pumped near 60 Hz.
        let timeout = if editor.game.is_some() {
            Duration::from_millis(8)
        } else {
            Duration::from_millis(50)
        };
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
        {
            editor.handle_key(key);
        }
    }
    Ok(())
}

fn forward(stream: impl io::Read + Send + 'static, sender: Sender<String>) {
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
}

fn styled(line: &str) -> Vec<Span<'static>> {
    highlight(line)
        .into_iter()
        .map(|(token, text)| {
            let style = match token {
                Token::Keyword => Style::default().fg(Color::Magenta),
                Token::Literal => Style::default().fg(Color::Cyan),
                Token::String => Style::default().fg(Color::Green),
                Token::Comment => Style::default().fg(Color::DarkGray),
                Token::Number => Style::default().fg(Color::Yellow),
                Token::Ident | Token::Punct => Style::default(),
            };
            Span::styled(text.to_string(), style)
        })
        .collect()
}

/// The project root the editor was opened on.
pub fn project_root(options: &EditorOptions) -> &Path {
    &options.project
}
