use ratatui::{
    Frame, 
    layout::{ Constraint, Direction, Layout},
    style::{Color, Style}, 
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap}
};
use crate::types::{AgentCommand, AppState};
use tokio::sync::mpsc;
use crossterm::event::{KeyCode, KeyEventKind, EventStream, Event};
use futures::StreamExt;
use crate::types::{Action, Message};

pub struct App {
    pub state: AppState,
    pub terminal: ratatui::DefaultTerminal,
    pub agent_tx: mpsc::Sender<AgentCommand>,
    pub ui_rx: mpsc::Receiver<Action>,
    pub should_quit: bool,
    pub events: EventStream,
}

impl App {
    pub fn new(
        terminal: ratatui::DefaultTerminal,
        agent_tx: mpsc::Sender<AgentCommand>,
        ui_rx: mpsc::Receiver<Action>,
    ) -> Self {
        Self {
            state: AppState::default(),
            terminal,
            agent_tx,
            ui_rx,
            should_quit: false,
            events: EventStream::new()
        }
    }

    pub async fn run(&mut self) -> std::io::Result<()> {
        self.terminal.clear()?;

        while !self.should_quit {
            self.draw()?;
            self.handle_events().await?;
        }

        Ok(())
    }

    async fn handle_events(&mut self) -> std::io::Result<()> {
        tokio::select! {
            Some(Ok(event)) = self.events.next() => {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Up => {
                                // Prevent going below 0
                                self.state.scroll_offset = self.state.scroll_offset.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                // Tradeoff: simple but if user presses down, it just goes down indefinitely
                                // Need to find a better way to handle this.
                                self.state.scroll_offset = self.state.scroll_offset.saturating_add(1);
                            }
                            KeyCode::Esc => {
                                self.should_quit = true;
                            }
                            KeyCode::Char(c) => { 
                                self.state.input_buffer.push(c);
                            }
                            KeyCode::Backspace => {
                                 self.state.input_buffer.pop(); 
                            }
                            KeyCode::Enter => {
                                if !self.state.input_buffer.is_empty() {
                                    let msg = std::mem::take(&mut self.state.input_buffer);
                                    self.state.chat_history.push(Message {
                                        role: "user".to_string(),
                                        content: msg
                                    });
                                    if self.agent_tx.send(AgentCommand::Run(self.state.chat_history.clone())).await.is_err() {
                                        // TODO: log something here?
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Some(action) = self.ui_rx.recv() => {
                match action {
                    Action::Response(msg) => {
                        self.state.chat_history.push(Message {
                            role: "assistant".to_string(),
                            content: msg
                        });
                    }
                    Action::Stream(token) => {
                        if let Some(last_msg) = self.state.chat_history.last_mut() {
                            if last_msg.role == "assistant" {
                                last_msg.content.push_str(&token);
                            } else {
                                self.state.chat_history.push(Message {
                                    role: "assistant".to_string(),
                                    content: token
                                });
                            }
                        } else {
                            self.state.chat_history.push(Message {
                                role: "assistant".to_string(),
                                content: token
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn build_ui(frame: &mut Frame, state: &AppState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(3)
            ])
            .split(frame.area());

        // Add chat log
        let history_text: Vec<Line> = state.chat_history
            .iter()
            .map(|msg| {
                let (prefix, color) = if msg.role == "user" {
                    ("You: ", Color::Blue)
                } else {
                    ("AI: ", Color::Green)
                };

                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(color).bold()),
                    Span::raw(&msg.content)
                ])
            })
            .collect();

        let history = Paragraph::new(history_text)
            .block(Block::default().title("Chat").borders(Borders::ALL))
            .wrap(Wrap { trim: true })
            .scroll((state.scroll_offset, 0));

        frame.render_widget(history, chunks[0]);

        // Input
        let input = Paragraph::new(state.input_buffer.as_str())
            .block(Block::default().title(" Input ").borders(Borders::ALL));
        frame.render_widget(input, chunks[1]);
    }

    fn draw(&mut self) -> std::io::Result<()> {
        self.terminal.draw(|f| Self::build_ui(f, &self.state))?;
        Ok(())
    }
}