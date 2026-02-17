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
        let prompt = "You are an advanced Rust coding agent. \
        You are designed to help the user understand and write Rust code.\n\
        You keep your responses short, efficient and concise\n\
        \n\
        TOOLS:\n\
        You have access to a local filesystem. You can read files to understand the codebase.\n\
        \n\
        To read a file, you MUST output a tool call in this exact format:\n\
        <read_file>src/main.rs</read_file>\n\
        \n\
        RULES:\n\
        1. Only read one file at a time.\n\
        2. After you output the <read_file> tag, STOP generating text immediately. \
           Wait for the system to provide the file content.\n\
        3. Do not hallucinate the file content. \
           If you need to see a file, ask for it using the tool.\n\
        \n\
        EXAMPLE:\n\
        User: 'How does the main loop work?'\n\
        Assistant: <read_file>src/main.rs</read_file>\n\
        System: (Returns file content...)\n\
        Assistant: 'The main loop handles events by...'";

        let system_message = Message {
            role: "system".to_string(),
            content: prompt.to_string()
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
                                                if tx.send(Action::Done).await.is_err() {
                                                    // TODO: need to add logging?
                                                }
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