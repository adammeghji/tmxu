use std::process::Command;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use tui_tree_widget::{Tree, TreeItem};

use crate::app::{App, Mode};
use crate::tmux::{self, TmuxSession};

// BBS/warez color palette
const CYAN: Color = Color::Cyan;
const MAGENTA: Color = Color::Magenta;
const GREEN: Color = Color::Green;
const YELLOW: Color = Color::Yellow;
const DIM: Color = Color::DarkGray;
const WHITE: Color = Color::White;

/// Render the hostname banner once using tui-banner with Royal Purple style.
/// Returns ratatui Text for embedding in the header widget.
pub fn render_banner() -> Text<'static> {
    use ansi_to_tui::IntoText;

    let hostname = get_hostname();

    let ansi = match tui_banner::Banner::new(&hostname) {
        Ok(banner) => banner
            .style(tui_banner::Style::RoyalPurple)
            .padding((1, 0, 0, 2))
            .render(),
        Err(_) => hostname.clone(),
    };

    let mut text = ansi.into_text().unwrap_or_else(|_| Text::raw(hostname));
    // Trim trailing blank lines from tui-banner output
    while text.lines.last().is_some_and(|l| l.spans.iter().all(|s| s.content.trim().is_empty())) {
        text.lines.pop();
    }
    text
}

fn get_hostname() -> String {
    Command::new("hostname")
        .arg("-s")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "tmu".to_string())
}

/// Main draw function — renders entire UI from app state
pub fn draw(frame: &mut Frame, app: &mut App) {
    let (tree_area, status_area) = if let Some(ref banner) = app.banner {
        let header_height = banner.height() as u16 + 1; // +1 for bottom border
        let chunks = Layout::vertical([
            Constraint::Length(header_height),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(frame.area());
        draw_header(frame, banner, chunks[0]);
        (chunks[1], chunks[2])
    } else {
        let chunks = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(frame.area());
        (chunks[0], chunks[1])
    };

    draw_tree(frame, app, tree_area);
    draw_status_bar(frame, app, status_area);

    // Draw popups on top
    match &app.mode {
        Mode::CreateSession { input } => {
            draw_input_popup(frame, "New Session", input);
        }
        Mode::RenameSession { target, input } => {
            let title = format!("Rename '{target}'");
            draw_input_popup(frame, &title, input);
        }
        Mode::ConfirmKill { target } => {
            draw_confirm_popup(frame, target);
        }
        Mode::Normal => {}
    }
}

fn draw_header(frame: &mut Frame, banner: &Text, area: Rect) {
    let header = Paragraph::new(banner.clone()).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(DIM)),
    );
    frame.render_widget(header, area);
}

fn draw_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.sessions.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No tmux sessions found.",
                Style::default().fg(DIM),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press n to create a new session.",
                Style::default().fg(YELLOW),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(DIM)),
        );
        frame.render_widget(empty, area);
        return;
    }

    let items = build_tree_items(&app.sessions);
    let tree = Tree::new(&items)
        .expect("unique identifiers")
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(DIM)),
        )
        .highlight_style(
            Style::default()
                .fg(WHITE)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ")
        .node_closed_symbol("▸ ")
        .node_open_symbol("▾ ")
        .node_no_children_symbol("  ");

    frame.render_stateful_widget(tree, area, &mut app.tree_state);
}

/// Map session index (0-based) to a label letter A-Z
fn session_label(idx: usize) -> char {
    if idx < 26 {
        (b'A' + idx as u8) as char
    } else {
        '?'
    }
}

/// Build tree items from session data for the tree widget
fn build_tree_items(sessions: &[TmuxSession]) -> Vec<TreeItem<'static, String>> {
    sessions
        .iter()
        .enumerate()
        .map(|(si, session)| {
            let label = session_label(si);

            let label_span = Span::styled(
                format!("[{label}] "),
                Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
            );

            let status = if session.attached {
                Span::styled("● ", Style::default().fg(Color::Green))
            } else {
                Span::styled("○ ", Style::default().fg(Color::DarkGray))
            };

            let name = Span::styled(
                session.name.clone(),
                Style::default()
                    .fg(CYAN)
                    .add_modifier(Modifier::BOLD),
            );

            let meta = Span::styled(
                format!("  ({} win)", session.window_count),
                Style::default().fg(DIM),
            );

            let attached_badge = if session.attached {
                Span::styled("  [attached]", Style::default().fg(GREEN))
            } else {
                Span::raw("")
            };

            let session_line = Line::from(vec![label_span, status, name, meta, attached_badge]);

            let window_items: Vec<TreeItem<'static, String>> = session
                .windows
                .iter()
                .enumerate()
                .map(|(wi, window)| {
                    let win_label = Span::styled(
                        format!("[{}] ", wi + 1),
                        Style::default().fg(YELLOW),
                    );
                    let summary = TmuxSession::window_summary(window);
                    let wname = Span::styled(
                        window.name.to_string(),
                        Style::default().fg(WHITE),
                    );
                    let path = Span::styled(
                        format!("  {summary}"),
                        Style::default().fg(DIM),
                    );
                    let window_line = Line::from(vec![win_label, wname, path]);

                    if window.panes.len() > 1 {
                        let pane_items: Vec<TreeItem<'static, String>> = window
                            .panes
                            .iter()
                            .map(|pane| {
                                let active_marker = if pane.active { "* " } else { "  " };
                                let pane_text = format!(
                                    "{}pane {}: {}  {}",
                                    active_marker,
                                    pane.index,
                                    pane.current_command,
                                    tmux::shorten_path(&pane.current_path),
                                );
                                TreeItem::new_leaf(
                                    format!("{}", pane.index),
                                    Span::styled(pane_text, Style::default().fg(DIM)),
                                )
                            })
                            .collect();
                        TreeItem::new(
                            format!("{}", window.index),
                            window_line,
                            pane_items,
                        )
                        .expect("unique pane identifiers")
                    } else {
                        TreeItem::new_leaf(format!("{}", window.index), window_line)
                    }
                })
                .collect();

            TreeItem::new(session.name.clone(), session_line, window_items)
                .expect("unique window identifiers")
        })
        .collect()
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let flash_line = if let Some(ref flash) = app.flash {
        Line::from(Span::styled(
            format!("  {}", flash.text),
            Style::default().fg(YELLOW),
        ))
    } else {
        Line::from("")
    };

    let keybinds = Line::from(vec![
        Span::styled("  a-z", Style::default().fg(CYAN)),
        Span::styled(":select  ", Style::default().fg(DIM)),
        Span::styled("A-Z", Style::default().fg(CYAN)),
        Span::styled(":open  ", Style::default().fg(DIM)),
        Span::styled("1-9", Style::default().fg(CYAN)),
        Span::styled(":window  ", Style::default().fg(DIM)),
        Span::styled("Enter", Style::default().fg(CYAN)),
        Span::styled(":attach  ", Style::default().fg(DIM)),
        Span::styled("n", Style::default().fg(CYAN)),
        Span::styled(":new  ", Style::default().fg(DIM)),
        Span::styled("d", Style::default().fg(CYAN)),
        Span::styled(":kill  ", Style::default().fg(DIM)),
        Span::styled("q", Style::default().fg(CYAN)),
        Span::styled(":quit", Style::default().fg(DIM)),
    ]);

    let status = Paragraph::new(vec![flash_line, keybinds]).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(DIM)),
    );
    frame.render_widget(status, area);
}

fn draw_input_popup(frame: &mut Frame, title: &str, input: &str) {
    let area = centered_rect(50, 5, frame.area());
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  > ", Style::default().fg(CYAN)),
            Span::styled(input, Style::default().fg(WHITE)),
            Span::styled("█", Style::default().fg(CYAN)), // cursor
        ]),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::bordered()
                .title(format!(" {title} "))
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(MAGENTA)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(popup, area);
}

fn draw_confirm_popup(frame: &mut Frame, target: &str) {
    let area = centered_rect(50, 5, frame.area());
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Kill session ", Style::default().fg(WHITE)),
            Span::styled(
                format!("'{target}'"),
                Style::default()
                    .fg(YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("? ", Style::default().fg(WHITE)),
            Span::styled("[y/N]", Style::default().fg(CYAN)),
        ]),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::bordered()
                .title(" Confirm Kill ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Red)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(popup, area);
}

/// Create a centered rectangle of given percentage width and fixed height
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);

    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1]);

    horizontal[1]
}
