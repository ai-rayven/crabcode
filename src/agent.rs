use tokio::sync::mpsc;
use futures::{StreamExt, TryStreamExt};
use tokio_util::codec::{FramedRead, LinesCodec};
use crate::types::{Action, AgentCommand};
use crate::types::{OllamaRequest, Message, OllamaResponse};

pub struct CodingAgent {
    client: reqwest::Client,
    model: String,
    api_url: String,
}

impl CodingAgent {
    pub fn new(
        model: String
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            model,
            api_url: "http://localhost:11434/api/chat".to_string(),
        }
    }

    pub async fn run(
        self,
        mut rx: mpsc::Receiver<AgentCommand>,
        tx: mpsc::Sender<Action>
    )
    {
        // NOTE: could define as const but this will be dynamic later so leaving here for now.
        let system_message = Message {
            role: "system".to_string(),
            content: "You are a helpful assistant".to_string()
        };

        while let Some(command) = rx.recv().await {
            match command {
                AgentCommand::Run(history) => {
                    let mut messages = vec![system_message.clone()];
                    messages.extend(history);

                    let request_body = OllamaRequest {
                        model: self.model.clone(),
                        messages: messages,
                        stream: true
                    };

                    let res = self.client.post(self.api_url.clone())
                        .json(&request_body)
                        .send()
                        .await;
                    
                    match res {
                        Ok(response) => {
                            let stream = response.bytes_stream();
                            let io_stream = stream.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
                            let reader = tokio_util::io::StreamReader::new(io_stream);
                            let mut lines = FramedRead::new(reader, LinesCodec::new());
                            let mut final_answer = String::new();

                            while let Some(line_result) = lines.next().await {
                                match line_result {
                                    Ok(line) => {
                                        if let Ok(parsed) = serde_json::from_str::<OllamaResponse>(&line) {
                                            if let Some(msg) = parsed.message {
                                                final_answer.push_str(&msg.content);
                                                if tx.send(Action::Stream(msg.content)).await.is_err() {
                                                    // TODO: need to add some logging or some way to not just lose this error?
                                                    break;
                                                }
                                            }
                                            if parsed.done {
                                                // done streaming, this is here to make intent clear and avoid compiler error of dead code for now.
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let err_msg = format!("Stream Error: {}", e);
                                        if tx.send(Action::Response(err_msg)).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let err_msg = format!("Network Error: {}", e);
                            if tx.send(Action::Response(err_msg)).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }

        }
    }
}