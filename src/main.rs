mod cli;
mod config;
mod error;
mod orchestrator;
mod prompt;
mod provider;
mod throbber;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.prompt.is_empty() {
        Cli::parse_from(["cmdify", "--help"]);
        return;
    }

    let user_prompt = cli.user_prompt();

    let config = match config::Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let throbber = throbber::Throbber::start();

    let result = orchestrator::run(&user_prompt, &config).await;

    throbber.stop();

    match result {
        Ok(content) => println!("{}", content),
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
