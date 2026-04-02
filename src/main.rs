mod cli;
mod config;
mod error;
mod orchestrator;
mod prompt;
mod provider;
mod spinner;

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

    let spinner = cli
        .spinner
        .or_else(|| {
            std::env::var("CMDIFY_SPINNER")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(1);

    let spinner_handle = spinner::Spinner::start(spinner);

    let result = orchestrator::run(&user_prompt, &config).await;

    spinner_handle.stop();

    match result {
        Ok(content) => {
            println!("{}", content);
            if cli.yolo {
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
                let status = std::process::Command::new(shell)
                    .arg("-c")
                    .arg(&content)
                    .status();
                match status {
                    Ok(s) => std::process::exit(s.code().unwrap_or(1)),
                    Err(e) => {
                        eprintln!("error executing command: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
