use crate::app::App;
use crate::ui::components;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(1), // Footer
        ])
        .split(frame.size());

    components::render_header(frame, chunks[0], "FENRIS HELP", app.connected);

    render_help_content(frame, chunks[1]);

    components::render_help_text(frame, chunks[2], &[("F1/Esc", "Back"), ("Ctrl+C", "Quit")]);
}

fn render_help_content(frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Commands list
        ])
        .split(area);

    let title = Paragraph::new(vec![
        Line::from(Span::styled(
            "FENRIS - Fast Encrypted Networked Robust Information Storage",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Available Commands: "),
    ])
    .alignment(Alignment::Center);

    frame.render_widget(title, chunks[0]);

    let commands = vec![
        ("ping", "Test connection to server"),
        ("ls [dir]", "List directory contents"),
        ("cd <dir>", "Change directory"),
        ("read <file>", "Read file contents"),
        ("write <file>", "Write to file"),
        ("create <file>", "Create new file"),
        ("rm <file>", "Delete file"),
        ("mkdir <dir>", "Create directory"),
        ("rmdir <dir>", "Delete directory"),
        ("info <file>", "Get file information"),
        ("help", "Show this help"),
        ("exit", "Disconnect and quit"),
    ];

    let items: Vec<ListItem> = commands
        .iter()
        .map(|(cmd, desc)| {
            let line = Line::from(vec![
                Span::styled(
                    format!(" {:20}", cmd),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(desc.to_string()),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" Commands "));

    frame.render_widget(list, chunks[1]);
}

use ratatui::layout::Rect;
