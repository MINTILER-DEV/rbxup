mod app;
mod cli;
mod config;
mod doctor;
mod error;
mod output;

use clap::Parser;

use crate::app::run;
use crate::cli::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(error) = run(cli).await {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }
}
