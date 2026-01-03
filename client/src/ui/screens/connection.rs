use crate::app::{App, ConnectionFocus};
use crate::ui::components;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Footer
        ])
        .split(frame.size());

    components::render_header(frame, chunks[0], "FENRIS CLIENT", app.connected);

    render_connection_form(frame, chunks[1], app);

    components::render_help_text(
        frame,
        chunks[2],
        &[
            ("Tab", "Switch field"),
            ("Enter", "Connect"),
            ("F1", "Help"),
            ("Ctrl+C", "Quit"),
        ],
    );
}

fn render_connection_form(frame: &mut Frame, area: Rect, app: &App) {
    let form_width = 60;
    let form_height = 15;

    let centered = center_rect(area, form_width, form_height);

    frame.render_widget(Clear, centered);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Spacing
            Constraint::Length(3), // Address input
            Constraint::Length(3), // Port input
            Constraint::Length(1), // Spacing
            Constraint::Min(0),    // Instructions
        ])
        .split(centered);

    let border = Block::default()
        .title(" Connect to Server ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(border, centered);

    let title = Paragraph::new("Enter server connection details")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(title, chunks[0]);

    let address_focused = matches!(app.connection_focus, ConnectionFocus::Address);
    let address_style = if address_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let address_title = " Server Address ";

    let address_block = Block::default()
        .title(address_title)
        .borders(Borders::ALL)
        .border_style(if address_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let address_input = Paragraph::new(app.server_addr.as_str())
        .style(address_style)
        .block(address_block);

    frame.render_widget(address_input, chunks[2]);

    let port_focused = matches!(app.connection_focus, ConnectionFocus::Port);
    let port_style = if port_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let port_title = " Port ";

    let port_block = Block::default()
        .title(port_title)
        .borders(Borders::ALL)
        .border_style(if port_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let port_input = Paragraph::new(app.server_port.as_str())
        .style(port_style)
        .block(port_block);

    frame.render_widget(port_input, chunks[3]);

    let instructions = vec![Line::from(vec![
        Span::raw("Use "),
        Span::styled(
            "Tab",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to switch fields, "),
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to connect"),
    ])];

    let instructions_paragraph = Paragraph::new(instructions)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(instructions_paragraph, chunks[5]);
}

fn center_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
