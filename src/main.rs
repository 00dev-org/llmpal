use clap::Parser;
use std::process;

use llmpal::{app, config};

#[tokio::main]
async fn main() {
    let args = config::Cli::parse();
    
    if let Err(e) = app::run(&args).await {
        eprintln!("{}", e);
        process::exit(1);
    }
}