use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Text;
use tui_tree_widget::TreeState;

use crate::tmux::{self, TmuxSession};
use crate::ui;

/// Application mode
#[derive(Debug, Clone)]
pub enum Mode {
    Normal,
    CreateSession { input: String },
    RenameSession { target: String, input: String },
    ConfirmKill { target: String },
}

/// Actions produced by key handling
#[derive(Debug)]
pub enum Action {
    Quit,
    Attach(String),
    Refresh,
    None,
}

/// Flash message shown in the status bar
#[derive(Debug, Clone)]
pub struct FlashMessage {
    pub text: String,
    pub created: Instant,
}

impl FlashMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            created: Instant::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created.elapsed().as_secs() >= 3
    }
}

const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// Main application state (Model)
pub struct App {
    pub sessions: Vec<TmuxSession>,
    pub tree_state: TreeState<String>,
    pub mode: Mode,
    pub flash: Option<FlashMessage>,
    pub banner: Option<Text<'static>>,
    last_refresh: Instant,
}

impl App {
    pub fn new(no_logo: bool) -> Result<Self> {
        let sessions = tmux::fetch_sessions().unwrap_or_default();
        let banner = if no_logo {
            None
        } else {
            Some(ui::render_banner())
        };
        let mut app = Self {
            sessions,
            tree_state: TreeState::default(),
            mode: Mode::Normal,
            flash: None,
            banner,
            last_refresh: Instant::now(),
        };
        if let Some(session) = app.sessions.first() {
            // Open the first session and select its first window
            app.tree_state.open(vec![session.name.clone()]);
            if let Some(window) = session.windows.first() {
                app.tree_state
                    .select(vec![session.name.clone(), format!("{}", window.index)]);
            } else {
                app.tree_state.select_first();
            }
        }
        Ok(app)
    }

    /// Refresh session data from tmux
    pub fn refresh(&mut self) {
        self.last_refresh = Instant::now();
        match tmux::fetch_sessions() {
            Ok(sessions) => {
                self.sessions = sessions;
            }
            Err(e) => {
                self.flash = Some(FlashMessage::new(format!("Refresh failed: {e}")));
            }
        }
    }

    /// Periodic housekeeping: expire flash messages, auto-refresh sessions
    pub fn tick(&mut self) {
        if let Some(ref flash) = self.flash {
            if flash.is_expired() {
                self.flash = None;
            }
        }

        if self.last_refresh.elapsed() >= AUTO_REFRESH_INTERVAL {
            self.refresh();
        }
    }

    /// Handle a key event and return an Action
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Action {
        match &self.mode {
            Mode::Normal => self.handle_normal_key(key),
            Mode::CreateSession { .. } => self.handle_create_session_key(key),
            Mode::RenameSession { .. } => self.handle_rename_session_key(key),
            Mode::ConfirmKill { .. } => self.handle_confirm_kill_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,

            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                self.tree_state.key_down();
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.tree_state.key_up();
                Action::None
            }
            KeyCode::Char('g') => {
                self.tree_state.select_first();
                Action::None
            }
            KeyCode::Char('G') => {
                self.tree_state.select_last();
                Action::None
            }

            // Expand / Collapse
            KeyCode::Char(' ') | KeyCode::Char('l') | KeyCode::Right => {
                self.tree_state.key_right();
                Action::None
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.tree_state.key_left();
                Action::None
            }

            // Attach
            KeyCode::Enter => self.action_attach(),

            // Session management
            KeyCode::Char('n') => {
                self.mode = Mode::CreateSession {
                    input: String::new(),
                };
                Action::None
            }
            KeyCode::Char('d') => self.action_start_kill(),
            KeyCode::Char('r') => self.action_start_rename(),
            KeyCode::Char('R') => Action::Refresh,

            // Shift+letter: attach to session immediately
            KeyCode::Char(c @ 'A'..='Z') => {
                self.jump_to_session(c);
                self.action_attach()
            }

            // Lowercase letter: navigate to session
            KeyCode::Char(c @ 'a'..='z') => {
                self.jump_to_session(c.to_ascii_uppercase());
                Action::None
            }

            // Jump to window by number 1-9
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_window(c);
                Action::None
            }

            _ => Action::None,
        }
    }

    /// Jump to session by label letter (A=0, B=1, ...)
    fn jump_to_session(&mut self, letter: char) {
        let idx = (letter as u8 - b'A') as usize;
        if let Some(session) = self.sessions.get(idx) {
            self.tree_state.open(vec![session.name.clone()]);
            // Select the first window in that session
            if let Some(window) = session.windows.first() {
                self.tree_state
                    .select(vec![session.name.clone(), format!("{}", window.index)]);
            } else {
                self.tree_state.select(vec![session.name.clone()]);
            }
        }
    }

    /// Jump to window N (1-based) within the currently selected session
    fn jump_to_window(&mut self, digit: char) {
        let win_display_idx = (digit as u8 - b'0') as usize; // 1-based display index
        let session_name = {
            let selected = self.tree_state.selected();
            if selected.is_empty() {
                return;
            }
            selected[0].clone()
        };

        let session = match self.sessions.iter().find(|s| s.name == session_name) {
            Some(s) => s,
            None => return,
        };

        // win_display_idx is 1-based positional (1st window, 2nd window, ...)
        if let Some(window) = session.windows.get(win_display_idx - 1) {
            self.tree_state.open(vec![session_name.clone()]);
            self.tree_state
                .select(vec![session_name, format!("{}", window.index)]);
        }
    }

    fn handle_create_session_key(&mut self, key: KeyEvent) -> Action {
        let Mode::CreateSession { ref mut input } = self.mode else {
            return Action::None;
        };

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                let name = input.trim().to_string();
                if name.is_empty() {
                    self.mode = Mode::Normal;
                    return Action::None;
                }
                match tmux::create_session(&name) {
                    Ok(()) => {
                        self.flash = Some(FlashMessage::new(format!("Created session '{name}'")));
                        self.mode = Mode::Normal;
                        return Action::Refresh;
                    }
                    Err(e) => {
                        self.flash = Some(FlashMessage::new(format!("Error: {e}")));
                        self.mode = Mode::Normal;
                    }
                }
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) => {
                input.push(c);
            }
            _ => {}
        }
        Action::None
    }

    fn handle_rename_session_key(&mut self, key: KeyEvent) -> Action {
        let Mode::RenameSession {
            ref target,
            ref mut input,
        } = self.mode
        else {
            return Action::None;
        };

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                let new_name = input.trim().to_string();
                let old_name = target.clone();
                if new_name.is_empty() || new_name == old_name {
                    self.mode = Mode::Normal;
                    return Action::None;
                }
                match tmux::rename_session(&old_name, &new_name) {
                    Ok(()) => {
                        self.flash = Some(FlashMessage::new(format!(
                            "Renamed '{old_name}' → '{new_name}'"
                        )));
                        self.mode = Mode::Normal;
                        return Action::Refresh;
                    }
                    Err(e) => {
                        self.flash = Some(FlashMessage::new(format!("Error: {e}")));
                        self.mode = Mode::Normal;
                    }
                }
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) => {
                input.push(c);
            }
            _ => {}
        }
        Action::None
    }

    fn handle_confirm_kill_key(&mut self, key: KeyEvent) -> Action {
        let Mode::ConfirmKill { ref target } = self.mode else {
            return Action::None;
        };

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let target = target.clone();
                match tmux::kill_session(&target) {
                    Ok(()) => {
                        self.flash = Some(FlashMessage::new(format!("Killed session '{target}'")));
                        self.mode = Mode::Normal;
                        return Action::Refresh;
                    }
                    Err(e) => {
                        self.flash = Some(FlashMessage::new(format!("Error: {e}")));
                        self.mode = Mode::Normal;
                    }
                }
            }
            _ => {
                self.mode = Mode::Normal;
            }
        }
        Action::None
    }

    /// Determine attach target from current tree selection
    fn action_attach(&mut self) -> Action {
        let selected = self.tree_state.selected();
        if selected.is_empty() {
            return Action::None;
        }

        let target = match selected.len() {
            1 => selected[0].clone(),
            _ => format!("{}:{}", selected[0], selected[1]),
        };

        Action::Attach(target)
    }

    /// Start kill confirmation for the selected session
    fn action_start_kill(&mut self) -> Action {
        let selected = self.tree_state.selected();
        if selected.is_empty() {
            return Action::None;
        }
        let session_name = selected[0].clone();
        self.mode = Mode::ConfirmKill {
            target: session_name,
        };
        Action::None
    }

    /// Start rename for the selected session
    fn action_start_rename(&mut self) -> Action {
        let selected = self.tree_state.selected();
        if selected.is_empty() {
            return Action::None;
        }
        let session_name = selected[0].clone();
        self.mode = Mode::RenameSession {
            target: session_name.clone(),
            input: session_name,
        };
        Action::None
    }
}
