use crate::app::{Message, MessageKind};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::time::Instant;

pub fn render_header(frame: &mut Frame, area: Rect, title: &str, connected: bool) {
    let status = if connected {
        Span::styled(" ● CONNECTED ", Style::default().fg(Color::Green))
    } else {
        Span::styled(" ● DISCONNECTED ", Style::default().fg(Color::Red))
    };

    let title_line = Line::from(vec![
        Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        status,
    ]);

    let header = Paragraph::new(title_line)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(header, area);
}

pub fn render_messages(frame: &mut Frame, area: Rect, messages: &[Message]) {
    let now = Instant::now();

    let lines: Vec<Line> = messages
        .iter()
        .rev()
        .take(area.height as usize - 2)
        .rev()
        .map(|msg| {
            let elapsed = now.duration_since(msg.timestamp).as_secs();
            let time_str = if elapsed < 60 {
                format!("[{}s ago]", elapsed)
            } else {
                format!("[{}m ago]", elapsed / 60)
            };

            let (icon, color) = match msg.kind {
                MessageKind::Info => ("ℹ", Color::Blue),
                MessageKind::Success => ("✓", Color::Green),
                MessageKind::Error => ("✗", Color::Red),
                MessageKind::Warning => ("⚠", Color::Yellow),
            };

            Line::from(vec![
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&msg.content),
                Span::styled(
                    format!(" {}", time_str),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        })
        .collect();

    let block = Block::default()
        .title(" Output ")
        .borders(Borders::ALL)
        .style(Style::default());

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

pub fn render_input(
    frame: &mut Frame,
    area: Rect,
    prompt: &str,
    input: &str,
    cursor_position: usize,
) {
    let input_text = format!("{}{}", prompt, input);

    let block = Block::default()
        .title(" Input ")
        .borders(Borders::ALL)
        .style(Style::default());

    let paragraph = Paragraph::new(input_text)
        .block(block)
        .style(Style::default());

    frame.render_widget(paragraph, area);

    let cursor_x = area.x + prompt.len() as u16 + cursor_position as u16 + 1;
    let cursor_y = area.y + 1;

    if cursor_x < area.x + area.width - 1 && cursor_y < area.y + area.height - 1 {
        frame.set_cursor(cursor_x, cursor_y);
    }
}

pub fn render_help_text(frame: &mut Frame, area: Rect, shortcuts: &[(&str, &str)]) {
    let help_spans: Vec<Span> = shortcuts
        .iter()
        .flat_map(|(key, desc)| {
            vec![
                Span::styled(
                    format!(" {} ", key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("{} │", desc)),
            ]
        })
        .collect();

    let help_line = Line::from(help_spans);

    let paragraph = Paragraph::new(help_line)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(paragraph, area);
}
