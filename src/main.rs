mod cli;
mod config;
mod error;
mod logger;
mod orchestrator;
mod prompt;
mod provider;
mod spinner;
mod tools;

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

    let mut config = match config::Config::from_env(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    config.quiet = cli.quiet || config.quiet;
    config.blind = cli.blind || config.blind;
    config.no_tools = cli.no_tools || config.no_tools;
    config.yolo = cli.yolo || config.yolo;

    let spinner = cli.spinner.unwrap_or(config.spinner);

    let yolo = config.yolo;

    let lg = logger::CmdifyLogger::new(&config.model_name, &config.provider_name);

    let spinner_handle = spinner::Spinner::start(spinner);

    let result = orchestrator::run(&user_prompt, &config, Some(&lg)).await;

    spinner_handle.stop();

    match result {
        Ok(content) => {
            println!("{}", content);
            if yolo {
                lg.log("output", &content);
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
