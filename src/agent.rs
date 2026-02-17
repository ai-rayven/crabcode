use tokio::sync::mpsc;
use futures::StreamExt;
use crate::types::{Action};
use crate::types::{OllamaRequest, Message, OllamaResponse};

pub struct CodingAgent {
    client: reqwest::Client,
    model: String,
    api_url: String,
    history: Vec<Message>
}

impl CodingAgent {
    pub fn new(
        model: String
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            model,
            api_url: "http://localhost:11434/api/chat".to_string(),
            history: vec![Message {
                role: "system".to_string(),
                content: "You are a helpful assistant".to_string()
            }]
        }
    }

    pub async fn run(
        mut self,
        mut rx: mpsc::Receiver<String>,
        tx: mpsc::Sender<Action>
    )
    {
        while let Some(prompt) = rx.recv().await {
            self.history.push(Message {
                role: "user".to_string(),
                content: prompt
            });

            let request_body = OllamaRequest {
                model: self.model.clone(),
                messages: self.history.clone(),
                stream: true
            };

            let res = self.client.post(self.api_url.clone())
                .json(&request_body)
                .send()
                .await;
            
            match res {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    let mut final_answer = String::new();
                    let mut buffer = String::new();

                    while let Some(item) = stream.next().await {
                        match item {
                            Ok(chunk) => {
                                let s = String::from_utf8_lossy(&chunk);
                                buffer.push_str(&s);

                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer.drain(..=pos).collect::<String>();

                                    if let Ok(parsed) = serde_json::from_str::<OllamaResponse>(&line) {
                                        if let Some(msg) = parsed.message {
                                            let token = msg.content;
                                            final_answer.push_str(&token);
                                            let _ = tx.send(Action::Stream(token)).await;
                                        }
                                        if parsed.done {

                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                self.history.pop();
                                let err_msg = format!("Stream Error: {}", e);
                                if tx.send(Action::Response(err_msg)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }

                    self.history.push(Message {
                        role: "assistant".to_string(),
                        content: final_answer
                    })
                }
                Err(e) => {
                    self.history.pop();
                    let err_msg = format!("Network Error: {}", e);
                    if tx.send(Action::Response(err_msg)).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}