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

fn print_tool_levels() {
    println!("cmdify tool levels (default: 1)");
    println!();
    println!("Level 0 — no tools");
    println!();
    println!("Level 1 — core:");
    println!("  ask_user                Ask the user a clarifying question");
    println!("  find_command            Check whether a command exists on the system");
    println!("  list_current_directory  List files in the current working directory (not yet implemented)");
    println!();
    println!("Level 2 — local:");
    println!("  command_help            Show help text for a command (optional grep filter) (not yet implemented)");
    println!("  list_any_directory      List files in any user-specified directory (not yet implemented)");
    println!("  pwd                     Print the current working directory (not yet implemented)");
    println!();
    println!("Level 3 — system:");
    println!("  get_env                 Read environment variables (not yet implemented)");
    println!("  list_processes          List running processes (not yet implemented)");
    println!();
    println!("Use -t N or --tools N to set the tool level (0-3).");
    println!("Use -q, -b, -n to disable individual tools or all tools.");
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.list_tools {
        print_tool_levels();
        std::process::exit(0);
    }

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
    if let Some(t) = cli.tool_level {
        sources.push(config::ConfigSource {
            key: "tool_level".into(),
            value: t.to_string(),
            source: "cli".into(),
        });
        config.tool_level = t.min(3);
    }

    config.quiet = cli.quiet || config.quiet;
    config.blind = cli.blind || config.blind;
    config.no_tools = cli.no_tools || config.no_tools;
    config.yolo = cli.yolo || config.yolo;
    config.debug_level = std::cmp::max(config.debug_level, cli.debug);

    debug::init(config.debug_level);

    if !sources.iter().any(|s| s.key == "tool_level") {
        sources.push(config::ConfigSource {
            key: "tool_level".into(),
            value: config.tool_level.to_string(),
            source: "default".into(),
        });
    }

    for src in &sources {
        debug!("Config: {} = {} ({})", src.key, src.value, src.source);
    }

    let spinner = cli.spinner.unwrap_or(config.spinner);

    let yolo = config.yolo;

    let lg = logger::CmdifyLogger::new(&config.model_name, &config.provider_name);

    let spinner_handle = spinner::Spinner::start(spinner);
    let pause_handle = spinner_handle.pause_handle();

    let result = orchestrator::run(&user_prompt, &config, Some(&lg), Some(&pause_handle)).await;

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
