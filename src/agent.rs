use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::io::StreamReader;
use futures::{StreamExt, TryStreamExt};
use reqwest::Url;
use anyhow::{Result, Context};
use tracing::{info, error, debug, warn, instrument};

use crate::types::{Action, AgentCommand, OllamaRequest, Message, OllamaResponse};

pub const SYSTEM_PROMPT: &str = include_str!("prompts/agent.md");

pub struct CodingAgent {
    client: reqwest::Client,
    model: String,
    api_url: Url,
}

impl CodingAgent {
    pub fn new(
        model: String
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            model,
            api_url: Url::parse("http://localhost:11434/api/chat").expect("Invalid URL"),
        }
    }

    #[instrument(skip(self, rx, tx), fields(model = %self.model))]
    pub async fn run(
        self,
        mut rx: mpsc::Receiver<AgentCommand>,
        tx: mpsc::Sender<Action>
    ) {
        info!("Agent started");

        let system_message = Message {
            role: "system".to_string(),
            content: SYSTEM_PROMPT.to_string()
        };

        while let Some(command) = rx.recv().await {
            match command {
                AgentCommand::Run(history) => {
                    info!(msg_count = history.len(), "Processing run command");

                    if let Err(e) = self.exec_run(&system_message, &history, &tx).await {
                        error!(%e, "Critical failure in agent loop");
                        let _ = tx.send(Action::Response(format!("Internal Error: {}", e))).await;
                    }
                }
            }
        }
        info!("Agent shutting down");
    }

    #[instrument(skip(self, tx, system_message, history))]
    async fn exec_run(
        &self,
        system_message: &Message,
        history: &[Message],
        tx: &mpsc::Sender<Action>
    ) -> Result<()> {

        debug!("Preparing request to Ollama");

        let mut messages = Vec::with_capacity(history.len() + 1);
        messages.push(system_message.clone());
        messages.extend_from_slice(history);

        let request_body = OllamaRequest {
            model: self.model.clone(),
            messages,
            stream: true
        };

        debug!(url=%self.api_url, "Sending HTTP request");
        let res = self.client.post(self.api_url.clone())
            .json(&request_body)
            .send()
            .await
            .context("Failed to connect to Ollama")?;

        Self::handle_response(res, tx).await
    }

    #[instrument(skip(response, tx))]
    async fn handle_response(response: reqwest::Response, tx: &mpsc::Sender<Action>) -> Result<()> {
        let status = response.status();
        if !status.is_success() {
            warn!(%status, "Ollama returned non-success status");
        }

        let stream = response.bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));

        let reader = StreamReader::new(stream);
        let mut lines = FramedRead::new(reader, LinesCodec::new());

        while let Some(line_result) = lines.next().await {
            let line = line_result.context("Failed to read line from stream")?;

            match serde_json::from_str::<OllamaResponse>(&line) {
                Ok(parsed) => {
                    if let Some(msg) = parsed.message {
                        tx.send(Action::Stream(msg.content)).await?;
                    }
                    if parsed.done {
                        tx.send(Action::Done).await?;
                        return Ok(());
                    }
                }
                Err(e) => {
                    error!(%e, raw_line = %line, "Failed to parse JSON chunk");
                }
            }
        }

        Ok(())
    }
}
