use tokio::sync::mpsc;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use tracing::info;

use crate::ui::{App};
use crate::agent::{CodingAgent};
use crate::types::{Action, AgentCommand};

mod types;
mod ui;
mod agent;
mod tools;

fn setup_tracing() -> WorkerGuard {
    let file_appender = tracing_appender::rolling::daily("logs", "crabcode.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
        )
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .init();

    std::panic::set_hook(Box::new(|panic_info| {
        tracing::error!("PANIC: {}", panic_info);
    }));

    guard
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = setup_tracing();

    info!("Application started");

    let (ui_tx, ui_rx) = mpsc::channel::<Action>(10);
    let (agent_tx, agent_rx) = mpsc::channel::<AgentCommand>(10);

    tokio::spawn(
        async move {
            let agent = CodingAgent::new("qwen2.5-coder:7b".to_string());
            agent.run(agent_rx, ui_tx).await;
        }
    );

    let terminal = ratatui::init();
    let mut app = App::new(terminal, agent_tx, ui_rx);

    let app_result = app.run().await;

    ratatui::restore();
    app_result?;

    info!("Application stopped");

    Ok(())
}