use std::process::Command;

use color_eyre::eyre::{eyre, Context, Result};

#[derive(Debug, Clone)]
pub struct TmuxPane {
    pub index: u32,
    pub current_command: String,
    pub current_path: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TmuxWindow {
    pub index: u32,
    pub name: String,
    pub active: bool,
    pub panes: Vec<TmuxPane>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TmuxSession {
    pub name: String,
    pub id: String,
    pub attached: bool,
    pub window_count: u32,
    pub created: u64,
    pub windows: Vec<TmuxWindow>,
}

impl TmuxSession {
    /// Short display path for a window's active pane
    pub fn window_summary(window: &TmuxWindow) -> String {
        let pane = window
            .panes
            .iter()
            .find(|p| p.active)
            .or(window.panes.first());

        match pane {
            Some(p) => {
                let path = shorten_path(&p.current_path);
                format!("{}  {}", p.current_command, path)
            }
            None => String::new(),
        }
    }
}

/// Shorten home directory to ~ in paths
pub fn shorten_path(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = path.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    path.to_string()
}

pub fn is_tmux_available() -> bool {
    Command::new("tmux").arg("list-sessions").output().is_ok()
}

#[allow(dead_code)]
pub fn is_tmux_server_running() -> bool {
    Command::new("tmux")
        .arg("list-sessions")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Fetch all sessions, windows, and panes in a single tmux call.
pub fn fetch_sessions() -> Result<Vec<TmuxSession>> {
    let format = "#{session_name}|#{session_id}|#{session_attached}|#{session_windows}|#{session_created}|#{window_index}|#{window_name}|#{window_active}|#{pane_index}|#{pane_current_command}|#{pane_current_path}|#{pane_active}";

    let output = Command::new("tmux")
        .args(["list-panes", "-aF", format])
        .output()
        .wrap_err("Failed to run tmux list-panes")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "no server running" or "no sessions" are not hard errors
        if stderr.contains("no server running") || stderr.contains("no sessions") {
            return Ok(Vec::new());
        }
        return Err(eyre!("tmux error: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_sessions(&stdout)
}

fn parse_sessions(output: &str) -> Result<Vec<TmuxSession>> {
    use std::collections::BTreeMap;

    // Group by session name, then by window index
    let mut session_map: BTreeMap<String, TmuxSession> = BTreeMap::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 12 {
            continue;
        }

        let session_name = parts[0].to_string();
        let session_id = parts[1].to_string();
        let session_attached = parts[2] != "0";
        let session_windows: u32 = parts[3].parse().unwrap_or(0);
        let session_created: u64 = parts[4].parse().unwrap_or(0);
        let window_index: u32 = parts[5].parse().unwrap_or(0);
        let window_name = parts[6].to_string();
        let window_active = parts[7] != "0";
        let pane_index: u32 = parts[8].parse().unwrap_or(0);
        let pane_current_command = parts[9].to_string();
        let pane_current_path = parts[10].to_string();
        let pane_active = parts[11].trim() != "0";

        let pane = TmuxPane {
            index: pane_index,
            current_command: pane_current_command,
            current_path: pane_current_path,
            active: pane_active,
        };

        let session = session_map
            .entry(session_name.clone())
            .or_insert_with(|| TmuxSession {
                name: session_name,
                id: session_id,
                attached: session_attached,
                window_count: session_windows,
                created: session_created,
                windows: Vec::new(),
            });

        // Find or create window
        if let Some(window) = session.windows.iter_mut().find(|w| w.index == window_index) {
            window.panes.push(pane);
        } else {
            session.windows.push(TmuxWindow {
                index: window_index,
                name: window_name,
                active: window_active,
                panes: vec![pane],
            });
        }
    }

    // Sort windows by index within each session
    let mut sessions: Vec<TmuxSession> = session_map.into_values().collect();
    for session in &mut sessions {
        session.windows.sort_by_key(|w| w.index);
        for window in &mut session.windows {
            window.panes.sort_by_key(|p| p.index);
        }
    }

    Ok(sessions)
}

pub fn create_session(name: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["new-session", "-d", "-s", name])
        .output()
        .wrap_err("Failed to create tmux session")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to create session: {}", stderr.trim()));
    }
    Ok(())
}

pub fn kill_session(name: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output()
        .wrap_err("Failed to kill tmux session")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to kill session: {}", stderr.trim()));
    }
    Ok(())
}

pub fn rename_session(old_name: &str, new_name: &str) -> Result<()> {
    let output = Command::new("tmux")
        .args(["rename-session", "-t", old_name, new_name])
        .output()
        .wrap_err("Failed to rename tmux session")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to rename session: {}", stderr.trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sessions() {
        let output = "dev|$0|1|2|1700000000|0|zsh|1|0|zsh|/home/user|1\n\
                       dev|$0|1|2|1700000000|1|make|0|0|make|/home/user/project|1\n\
                       scratch|$1|0|1|1700000001|0|vim|1|0|vim|/tmp|1\n\
                       scratch|$1|0|1|1700000001|0|vim|1|1|bash|/tmp|0\n";

        let sessions = parse_sessions(output).unwrap();
        assert_eq!(sessions.len(), 2);

        assert_eq!(sessions[0].name, "dev");
        assert!(sessions[0].attached);
        assert_eq!(sessions[0].windows.len(), 2);

        assert_eq!(sessions[1].name, "scratch");
        assert!(!sessions[1].attached);
        assert_eq!(sessions[1].windows.len(), 1);
        assert_eq!(sessions[1].windows[0].panes.len(), 2);
    }

    #[test]
    fn test_parse_empty() {
        let sessions = parse_sessions("").unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_shorten_path() {
        std::env::set_var("HOME", "/home/user");
        assert_eq!(shorten_path("/home/user/code"), "~/code");
        assert_eq!(shorten_path("/tmp/foo"), "/tmp/foo");
    }
}
