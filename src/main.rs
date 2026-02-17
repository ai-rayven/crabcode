mod types;
mod ui;
mod agent;

use tokio::sync::mpsc;
use crate::ui::{App};
use crate::agent::{CodingAgent};
use crate::types::{Action, AgentCommand};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (ui_tx, ui_rx) = mpsc::channel::<Action>(10);
    let (agent_tx, agent_rx) = mpsc::channel::<AgentCommand>(10);

    tokio::spawn(
        async move {
            let agent = CodingAgent::new("llama3".to_string());
            agent.run(agent_rx, ui_tx).await;
        }
    );

    let terminal = ratatui::init();
    let mut app = App::new(terminal, agent_tx, ui_rx);

    let app_result = app.run().await;

    ratatui::restore();
    app_result?;

    Ok(())
}