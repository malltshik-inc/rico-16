//! SPEC §16 — the ratatui editor.

use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use rico16_core::palette::{Palette, expand5};

use crate::TuiError;
use crate::buffer::TextBuffer;
use crate::highlight::{Token, highlight};
use crate::tree::{Entry, project_files};

/// How to start the editor.
#[derive(Debug, Clone)]
pub struct EditorOptions {
    /// The project root.
    pub project: PathBuf,
    /// The `rico16` binary to spawn for Run (SPEC §16.4: the game gets its own
    /// window, so it runs as a child process).
    pub program: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Tree,
    Editor,
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
        let was_running = self.child.is_some();
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
                if was_running {
                    // SPEC §16.3 — rebuild and hot reload by resetting the cart.
                    self.restart_cart();
                }
            }
            Err(e) => self.log(format!("save failed: {e}")),
        }
    }

    /// SPEC §16.3 — build-on-run: `rico16 run` semantics from the editor.
    fn run_cart(&mut self) {
        self.stop_cart();
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
                self.log("running");
            }
            Err(e) => self.log(format!("cannot run: {e}")),
        }
    }

    fn stop_cart(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
            self.log("stopped");
        }
    }

    fn restart_cart(&mut self) {
        self.run_cart();
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
        match key.code {
            KeyCode::Char('q') if ctrl => self.should_quit = true,
            KeyCode::Char('s') if ctrl => self.save(),
            KeyCode::F(5) => self.run_cart(),
            KeyCode::F(6) => self.stop_cart(),
            KeyCode::F(7) => self.restart_cart(),
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Tree => Focus::Editor,
                    Focus::Editor => Focus::Tree,
                };
            }
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
                Focus::Tree => {
                    self.selected = self.selected.saturating_sub(1);
                }
                Focus::Editor => self.buffer.move_up(),
            },
            KeyCode::Down => match self.focus {
                Focus::Tree => {
                    if self.selected + 1 < self.files.len() {
                        self.selected += 1;
                    }
                }
                Focus::Editor => self.buffer.move_down(),
            },
            KeyCode::Enter => self.open_selected(),
            _ => {}
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(8)])
            .split(frame.area());
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(30),
                Constraint::Min(20),
                Constraint::Length(34),
            ])
            .split(vertical[0]);

        self.draw_tree(frame, horizontal[0]);
        self.draw_editor(frame, horizontal[1]);
        self.draw_palette(frame, horizontal[2]);
        self.draw_console(frame, vertical[1]);
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
    editor.stop_cart();
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    editor: &mut Editor,
) -> Result<(), TuiError> {
    while !editor.should_quit {
        while let Ok(line) = editor.output.try_recv() {
            editor.log(line);
        }
        terminal.draw(|frame| editor.draw(frame))?;
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind != crossterm::event::KeyEventKind::Release
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
