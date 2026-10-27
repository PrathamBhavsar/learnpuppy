use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    lp_tui::cli::run(lp_tui::cli::Cli::parse()).await
}
