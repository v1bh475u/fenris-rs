use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use crate::{
    app::{App, Screen},
    connection_manager::{ConnectionManager, ServerInfo},
    ui,
};

pub struct Client {
    app: App,
    connection_manager: ConnectionManager,
}

impl Client {
    pub fn new() -> Self {
        Self {
            app: App::new(),
            connection_manager: ConnectionManager::default(),
        }
    }

    pub async fn run(&mut self, terminal: &mut ui::terminal::Tui) -> Result<()> {
        self.app.info("Welcome to Fenris Client!");
        self.app.info("Press F1 for help, Ctrl+C to quit.");

        loop {
            terminal.draw(|frame| ui::render(frame, &self.app))?;

            if let Some(crossterm::event::Event::Key(key)) =
                ui::poll_events(Duration::from_millis(100))?
                && key.kind == crossterm::event::KeyEventKind::Press
            {
                self.handle_key_event(key).await?;
            }

            self.app.tick();

            if self.app.should_quit {
                if self.connection_manager.is_connected() {
                    self.connection_manager.disconnect().await;
                }
                break;
            }
        }
        Ok(())
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.app.should_quit = true;
            return Ok(());
        }

        match self.app.screen {
            Screen::Connection => {
                if key.code == KeyCode::Enter {
                    self.handle_connect().await?;
                    return Ok(());
                }
            }
            Screen::Command => {
                if key.code == KeyCode::Enter && !self.app.command_input.is_empty() {
                    self.handle_command().await?;
                    return Ok(());
                }
            }
            Screen::Help => { /* No async actions needed here */ }
        }

        ui::handle_key_event(&mut self.app, key)?;

        Ok(())
    }

    async fn handle_connect(&mut self) -> Result<()> {
        let address = self.app.server_addr.trim().to_string();
        let port: u16 = match self.app.server_port.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                self.app.error("Invalid port number");
                return Ok(());
            }
        };

        self.app
            .info(format!("Connecting to {}:{}...", address, port));
        if let Err(e) = self
            .connection_manager
            .set_server_info(ServerInfo::new(address.clone(), port))
        {
            self.app.error(format!("Failed to set server info: {}", e));
            return Ok(());
        }
        match self.connection_manager.connect().await {
            Ok(()) => {
                self.app.connected = true;
                self.app
                    .success(format!("Connected to {}:{}", address, port));
                self.app.screen = Screen::Command;
            }
            Err(e) => {
                self.app.connected = false;
                self.app.error(format!("Connection failed: {}", e));
            }
        }

        Ok(())
    }

    async fn handle_command(&mut self) -> Result<()> {
        let command = self.app.take_command();
        self.app.add_to_history(command.clone());

        self.app.info(format!("> {}", command));

        if command.trim() == "exit" || command.trim() == "quit" {
            self.app.info("Disconnecting...");
            self.connection_manager.disconnect().await;
            self.app.connected = false;
            self.app.screen = Screen::Connection;
            return Ok(());
        }

        if command.trim() == "clear" {
            self.app.messages.clear();
            return Ok(());
        }

        if command.trim() == "help" {
            self.app.screen = Screen::Help;
            return Ok(());
        }

        match self.connection_manager.send_command(&command).await {
            Ok(formatted) => {
                if formatted.success {
                    self.app.success(formatted.message);
                } else {
                    self.app.error(formatted.message);
                }

                if let Some(details) = formatted.details {
                    for line in details.lines() {
                        self.app.info(line.to_string());
                    }
                }

                if let Some(ref dir) = formatted.current_dir {
                    self.app.current_dir = dir.clone();
                }
            }
            Err(e) => {
                self.app.error(format!("Command failed: {}", e));

                if matches!(e, common::FenrisError::ConnectionClosed) {
                    self.app.connected = false;
                    self.app.screen = Screen::Connection;
                }
            }
        }

        Ok(())
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
