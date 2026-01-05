use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Connection,
    Command,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionFocus {
    Address,
    Port,
}

pub struct App {
    pub screen: Screen,
    pub should_quit: bool,

    pub server_addr: String,
    pub server_port: String,
    pub connection_focus: ConnectionFocus,
    pub connected: bool,
    pub current_dir: String,

    pub command_input: String,
    pub command_history: Vec<String>,
    pub history_index: Option<usize>,

    pub messages: Vec<Message>,
    pub cursor_position: usize,
    pub last_tick: Instant,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub timestamp: Instant,
    pub kind: MessageKind,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Error,
    Success,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Connection,
            should_quit: false,
            server_addr: String::from("127.0.0.1"),
            server_port: String::from("5555"),
            connection_focus: ConnectionFocus::Address,
            connected: false,
            current_dir: String::from("/"),
            command_input: String::new(),
            command_history: Vec::new(),
            history_index: None,
            messages: Vec::new(),
            cursor_position: 0,
            last_tick: Instant::now(),
        }
    }

    pub fn add_message(&mut self, kind: MessageKind, content: String) {
        self.messages.push(Message {
            timestamp: Instant::now(),
            kind,
            content,
        });

        if self.messages.len() > 1000 {
            self.messages.drain(0..100);
        }
    }

    pub fn info(&mut self, content: impl Into<String>) {
        self.add_message(MessageKind::Info, content.into());
    }

    pub fn error(&mut self, content: impl Into<String>) {
        self.add_message(MessageKind::Error, content.into());
    }

    pub fn success(&mut self, content: impl Into<String>) {
        self.add_message(MessageKind::Success, content.into());
    }

    pub fn add_to_history(&mut self, command: String) {
        if !command.is_empty() {
            self.command_history.push(command);
            self.history_index = None;
        }
    }

    pub fn history_previous(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let index = match self.history_index {
            Some(0) | None => self.command_history.len() - 1,
            Some(i) => i - 1,
        };

        self.history_index = Some(index);
        self.command_input = self.command_history[index].clone();
        self.cursor_position = self.command_input.len();
    }

    pub fn history_next(&mut self) {
        match self.history_index {
            None => {}
            Some(i) => {
                if i < self.command_history.len() - 1 {
                    self.history_index = Some(i + 1);
                    self.command_input = self.command_history[i + 1].clone();
                    self.cursor_position = self.command_input.len();
                }
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.command_input.insert(self.cursor_position, c);
        self.cursor_position += 1;
        self.history_index = None;
    }

    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            self.command_input.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.command_input.len() {
            self.cursor_position += 1;
        }
    }

    pub fn move_cursor_start(&mut self) {
        self.cursor_position = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_position = self.command_input.len();
    }

    pub fn take_command(&mut self) -> String {
        let cmd = self.command_input.clone();
        self.command_input.clear();
        self.cursor_position = 0;
        self.history_index = None;
        cmd
    }

    pub fn tick(&mut self) {
        self.last_tick = Instant::now();
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
