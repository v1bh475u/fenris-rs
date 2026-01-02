pub mod components;
pub mod screens;
pub mod terminal;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use std::time::Duration;

use crate::app::{App, Screen};

pub fn render(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Connection => screens::connection::render(frame, app),
        Screen::Command => screens::command::render(frame, app),
        Screen::Help => screens::help::render(frame, app),
    }
}

pub fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return Ok(());
    }

    match app.screen {
        Screen::Connection => handle_connection_input(app, key),
        Screen::Command => handle_command_input(app, key),
        Screen::Help => handle_help_input(app, key),
    }
}

fn handle_connection_input(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Enter => {
            // Will be handled in main loop to actually connect
        }
        KeyCode::Char(c) => {
            app.server_addr.push(c);
        }
        KeyCode::Backspace => {
            app.server_addr.pop();
        }
        _ => {}
    }
    Ok(())
}

fn handle_command_input(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::F(1) => {
            app.screen = Screen::Help;
        }
        KeyCode::Enter => {
            // Command will be processed in main loop
        }
        KeyCode::Up => {
            app.history_previous();
        }
        KeyCode::Down => {
            app.history_next();
        }
        KeyCode::Left => {
            app.move_cursor_left();
        }
        KeyCode::Right => {
            app.move_cursor_right();
        }
        KeyCode::Home => {
            app.move_cursor_start();
        }
        KeyCode::End => {
            app.move_cursor_end();
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
        }
        KeyCode::Backspace => {
            app.delete_char();
        }
        _ => {}
    }
    Ok(())
}

fn handle_help_input(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::F(1) | KeyCode::Esc => {
            app.screen = Screen::Command;
        }
        _ => {}
    }
    Ok(())
}

pub fn poll_events(timeout: Duration) -> Result<Option<Event>> {
    if event::poll(timeout)? {
        Ok(Some(event::read()?))
    } else {
        Ok(None)
    }
}
