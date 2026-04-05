mod cli;
mod config;
mod debug;
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

    let (mut config, mut sources) = match config::Config::from_env(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if cli.quiet {
        sources.push(config::ConfigSource {
            key: "quiet".into(),
            value: "true".into(),
            source: "cli".into(),
        });
    }
    if cli.blind {
        sources.push(config::ConfigSource {
            key: "blind".into(),
            value: "true".into(),
            source: "cli".into(),
        });
    }
    if cli.no_tools {
        sources.push(config::ConfigSource {
            key: "no_tools".into(),
            value: "true".into(),
            source: "cli".into(),
        });
    }
    if cli.yolo {
        sources.push(config::ConfigSource {
            key: "yolo".into(),
            value: "true".into(),
            source: "cli".into(),
        });
    }
    if cli.debug > 0 {
        sources.push(config::ConfigSource {
            key: "debug".into(),
            value: cli.debug.to_string(),
            source: "cli".into(),
        });
    }
    if let Some(s) = cli.spinner {
        sources.push(config::ConfigSource {
            key: "spinner".into(),
            value: s.to_string(),
            source: "cli".into(),
        });
    }

    config.quiet = cli.quiet || config.quiet;
    config.blind = cli.blind || config.blind;
    config.no_tools = cli.no_tools || config.no_tools;
    config.yolo = cli.yolo || config.yolo;
    config.debug_level = std::cmp::max(config.debug_level, cli.debug);

    debug::init(config.debug_level);

    for src in &sources {
        debug!("Config: {} = {} ({})", src.key, src.value, src.source);
    }

    let spinner = cli.spinner.unwrap_or(config.spinner);

    let yolo = config.yolo;

    let lg = logger::CmdifyLogger::new(&config.model_name, &config.provider_name);

    let spinner_handle = spinner::Spinner::start(spinner);

    let result = orchestrator::run(&user_prompt, &config, Some(&lg)).await;

    spinner_handle.stop();

    match result {
        Ok(content) => {
            debug!("Final response: {}", content);
            println!("{}", content);
            if yolo {
                lg.log("output", &content);
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
                debug!("shell exec: {} -c {}", shell, content);
                let status = std::process::Command::new(shell)
                    .arg("-c")
                    .arg(&content)
                    .status();
                match status {
                    Ok(s) => {
                        let code = s.code().unwrap_or(1);
                        debug!("Yolo: exit code {}", code);
                        std::process::exit(code);
                    }
                    Err(e) => {
                        eprintln!("error executing command: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            debug!("Error: {}", e);
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
