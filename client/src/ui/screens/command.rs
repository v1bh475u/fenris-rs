use crate::app::App;
use crate::ui::components;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Messages
            Constraint::Length(3), // Input
            Constraint::Length(1), // Footer
        ])
        .split(frame.size());

    components::render_header(frame, chunks[0], "FENRIS CLIENT", app.connected);

    components::render_messages(frame, chunks[1], &app.messages);

    let prompt = format!("{} -> ", app.current_dir);
    components::render_input(
        frame,
        chunks[2],
        &prompt,
        &app.command_input,
        app.cursor_position,
    );

    let cursor_x = chunks[2].x + prompt.len() as u16 + app.cursor_position as u16 + 1;
    let cursor_y = chunks[2].y + 1;

    frame.set_cursor(cursor_x, cursor_y);

    components::render_help_text(
        frame,
        chunks[3],
        &[("F1", "Help"), ("↑↓", "History"), ("Ctrl+C", "Quit")],
    );
}
