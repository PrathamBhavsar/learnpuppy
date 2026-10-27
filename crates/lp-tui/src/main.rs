use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = lp_tui::cli::Cli::parse();
    // Logging is best effort: an unwritable state dir must not block the app (it reports its own).
    let _log = lp_tui::logging::init(&cli.state_root()?).ok();
    lp_tui::cli::run(cli).await
}
