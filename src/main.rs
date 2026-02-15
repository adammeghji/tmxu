mod app;
mod tmux;
mod ui;

use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::app::{Action, App};

fn main() -> Result<()> {
    color_eyre::install()?;

    let no_logo = std::env::args().any(|a| a == "--no-logo");

    // Check tmux is available
    if !tmux::is_tmux_available() {
        eprintln!("tmxu: tmux is not installed or not in PATH");
        std::process::exit(1);
    }

    let mut terminal = ratatui::init();
    terminal.clear()?;
    let result = run(&mut terminal, no_logo);
    ratatui::restore();

    // If we're attaching, exec into tmux after terminal cleanup
    match result {
        Ok(Some(target)) => exec_tmux_attach(&target),
        Ok(None) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Main event loop. Returns Some(target) if user wants to attach, None if quit.
fn run(terminal: &mut DefaultTerminal, no_logo: bool) -> Result<Option<String>> {
    let mut app = App::new(no_logo)?;

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        // Poll with timeout for tick-based updates (flash message expiry)
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events (not release/repeat)
                if key.kind == KeyEventKind::Press {
                    match app.handle_key_event(key) {
                        Action::Quit => return Ok(None),
                        Action::Attach(target) => return Ok(Some(target)),
                        Action::Refresh => app.refresh(),
                        Action::None => {}
                    }
                }
            }
        }

        app.tick();
    }
}

/// Replace current process with tmux attach. Never returns on success.
fn exec_tmux_attach(target: &str) -> Result<()> {
    let err = Command::new("tmux")
        .args(["attach-session", "-t", target])
        .exec();

    // exec() only returns if it fails
    Err(err.into())
}
