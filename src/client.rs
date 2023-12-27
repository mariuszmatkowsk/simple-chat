mod screen_state;
mod terminal_buffer;

use screen_state::ScreenState;

use terminal_buffer::{apply_patches, TerminalBuffer};

use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::Color,
    terminal, QueueableCommand,
};

use std::io::{self, stdout, Write};

use std::net::{TcpListener, TcpStream};
use std::time::Duration;

macro_rules! chat_msg {
    ($chat:expr, $($arg:tt)*) => {
        $chat.insert(format!($($arg)*), Color::White);
    }
}

macro_rules! chat_error {
    ($chat:expr, $($arg:tt)*) => {
        $chat.insert(format!($($arg)*), Color::Red);
    }
}

macro_rules! chat_info {
    ($chat:expr, $($arg:tt)*) => {
        $chat.insert(format!($($arg)*), Color::Blue);
    }
}

#[derive(Default)]
struct Prompt {
    data: Vec<char>,
    cursor: usize,
}

impl Prompt {
    fn insert(&mut self, ch: char) {
        self.data.insert(self.cursor, ch);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        self.cursor = if self.cursor > 1 { self.cursor - 1 } else { 0 };
        self.data.pop();
    }

    fn clear(&mut self) {
        self.cursor = 0;
        self.data.clear();
    }

    fn render(&self, buffer: &mut TerminalBuffer, x: usize, y: usize, w: usize) {
        let chars = &self.data;
        buffer.put_cells(chars, x, y, Color::White, Color::Black);

        for pos_x in chars.len()..w {
            buffer.put_cell(' ', pos_x, y, Color::White, Color::Black);
        }
    }

    fn cursor_move_left(&mut self) {
        self.cursor = if self.cursor > 0 { self.cursor - 1 } else { 0 };
    }

    fn cursor_move_right(&mut self) {
        self.cursor = if self.cursor == self.data.len() {
            self.cursor
        } else {
            self.cursor + 1
        };
    }

    fn sync_cursor_with_terminal(
        &self,
        qc: &mut impl Write,
        x: usize,
        y: usize,
        w: usize,
    ) -> io::Result<()> {
        let cursor_x_pos = std::cmp::min(x + self.cursor, w);
        qc.queue(cursor::MoveTo(cursor_x_pos as u16, y as u16))?;
        Ok(())
    }

    fn get(&self) -> String {
        self.data.iter().collect()
    }
}

#[derive(Default)]
struct ChatLog {
    items: Vec<(String, Color)>,
}

impl ChatLog {
    fn insert(&mut self, message: String, color: Color) {
        self.items.push((message, color))
    }

    fn render(&self, buffer: &mut TerminalBuffer, x: usize, y: usize) {
        for (dy, (message, color)) in self.items.iter().enumerate() {
            let message_chars: Vec<_> = message.chars().collect();
            buffer.put_cells(&message_chars, x, y + dy, *color, Color::Black);
        }
    }
}

fn status_bar(buffer: &mut TerminalBuffer, label: &str, x: usize, y: usize, w: usize) {
    let label_chars: Vec<_> = label.chars().collect();
    let n = std::cmp::min(label_chars.len(), w);

    buffer.put_cells(&label_chars[..n], x, y, Color::Black, Color::White);

    for pos_x in label_chars.len()..w {
        buffer.put_cell(' ', pos_x, y, Color::Black, Color::White);
    }
}

#[derive(Default)]
struct Client {
    stream: Option<TcpStream>,
    chat: ChatLog,
    quit: bool,
}

fn connect_command(client: &mut Client, argument: &str) {
    let ip = argument;
    if client.stream.is_none() {
        client.stream = TcpStream::connect(&format!("{ip}:6969")).and_then(|mut stream| {
            stream.set_nonblocking(true)?;
            Ok(stream)
        })
        .map_err(|err| {
            chat_error!(&mut client.chat, "Could not connect to {ip}: {err}");
        })
        .ok();
    } else {
        chat_error!(&mut client.chat, "You are already connected to a server. Disconnect with /disconnect first.");
    }
}

fn disconnect_command(client: &mut Client, _argument: &str) {
    if client.stream.is_some() {
        client.stream = None;
        chat_info!(&mut client.chat, "Disconnected.");
    } else {
        chat_info!(&mut client.chat, "You are already offline. To connect use /connect <ip>");
    }
}

fn quit_command(client: &mut Client, _argument: &str) {
    client.quit = true;
}

fn help_command(client: &mut Client, argument: &str) {
    let name = argument.trim();
    if name.is_empty() {
        for Command{signature, description, ..} in COMMANDS.iter() {
            chat_info!(client.chat, "{signature} - {description}");
        }
    } else {
        if let Some(Command{signature, description, ..}) = find_command(name) {
            chat_info!(client.chat, "{signature} - {description}");
        } else {
            chat_error!(client.chat, "Unknown command `/{name}`");
        }
    }
}


struct Command {
    name: &'static str,
    description: &'static str,
    signature: &'static str,
    run: fn(&mut Client, &str),
}

const COMMANDS: &[Command] = &[
    Command {
        name: "connect",
        run: connect_command,
        description: "Connect to a server by <ip>",
        signature: "/connect <ip>",
    },
    Command {
        name: "disconnect",
        run: disconnect_command,
        description: "Disconnect from the server which one you are currently connected to",
        signature: "/disconnect",
    },
    Command {
        name: "quit",
        run: quit_command,
        description: "Close the chat",
        signature: "/quit",
    },
    Command {
        name: "help",
        run: help_command,
        description: "Print help",
        signature: "/help [command]",
    },
];

fn find_command(name: &str) -> Option<&Command> {
    COMMANDS.iter().find(|command| command.name == name)
}

fn parse_prompt(prompt: &[char]) -> Option<(&[char], &[char])> {
    let prompt = prompt.strip_prefix(&['/'])?;
    let mut iter = prompt.splitn(2, |x| *x == ' ');
    let a = iter.next().unwrap_or(prompt);
    let b = iter.next().unwrap_or(&[]);
    Some((a, b))
}


fn main() -> io::Result<()> {
    let mut client = Client::default();
    let _screen_state = ScreenState::enable();
    let mut stdout = stdout();

    let mut prompt = Prompt::default();

    let (w, h) = terminal::size()?;

    let mut screen_buffer = TerminalBuffer::new(w as usize, h as usize);
    let mut prev_screen_buffer = TerminalBuffer::new(w as usize, h as usize);

    prev_screen_buffer.flush(&mut stdout)?;
    while !client.quit {
        if poll(Duration::ZERO)? {
            match read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    match key_event.code {
                        KeyCode::Char(ch) => {
                            if key_event.modifiers == KeyModifiers::CONTROL {
                                match ch {
                                    'c' => quit_command(&mut client, ""),
                                    'h' => prompt.cursor_move_left(),
                                    'l' => prompt.cursor_move_right(),
                                    _ => (),
                                }
                            } else {
                                prompt.insert(ch);
                            }
                        }
                        KeyCode::Enter => {
                            if let Some((command, argument)) = parse_prompt(&prompt.data) {
                                let command_name: String = command.iter().collect();
                                let argument: String = argument.iter().collect();
                                if let Some(command) = find_command(&command_name) {
                                    (command.run)(&mut client, &argument);
                                } else {
                                    chat_error!(&mut client.chat, "Unknown command `/{command_name}`");
                                }
                            } else {
                                if let Some(ref mut stream) = &mut client.stream {
                                    let prompt: String = prompt.data.iter().collect();
                                    stream.write(prompt.as_bytes())?;
                                    chat_msg!(&mut client.chat, "{text}", text=&prompt);
                                } else {
                                    chat_info!(
                                        &mut client.chat,
                                        "You are offline. Use {signature} to connect to a server.",
                                            signature = find_command("connect").expect("connect command").signature);
                                }
                            }


                            if client.stream.is_none() {
                                client.chat.insert("Before sending message you need use /connect <ip> <port> command...".to_string(), Color::Blue);
                            } else {
                                client.chat.insert(prompt.get(), Color::White);
                                prompt.clear();
                            }
                        }
                        KeyCode::Backspace => {
                            prompt.backspace();
                        }
                        KeyCode::Left => {
                            prompt.cursor_move_left();
                        }
                        KeyCode::Right => {
                            prompt.cursor_move_right();
                        }
                        KeyCode::Esc => {
                            prompt.clear();
                        }
                        _ => (),
                    }
                }
                // handle other events
                _ => (),
            }
        }

        screen_buffer.clear();

        status_bar(&mut screen_buffer, "simple-chat", 0, 0, w.into());

        client.chat.render(&mut screen_buffer, 0, 1);

        if let Some(y) = h.checked_sub(2) {
            status_bar(&mut screen_buffer, "Online", 0, y.into(), w.into())
        }

        if let Some(y) = h.checked_sub(1) {
            prompt.render(&mut screen_buffer, 0, y.into(), w.into());
        }

        let patches = screen_buffer.diff(&prev_screen_buffer);
        apply_patches(&mut stdout, &patches)?;

        if let Some(y) = h.checked_sub(1) {
            prompt.sync_cursor_with_terminal(&mut stdout, 0, y.into(), w.into())?;
        }

        stdout.flush()?;

        std::mem::swap(&mut screen_buffer, &mut prev_screen_buffer);

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
