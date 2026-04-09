mod app;
mod cli;
mod config;
mod debug;
mod error;
mod logger;
mod orchestrator;
mod prompt;
mod provider;
mod safety;
mod setup;
mod spinner;
mod tools;

use clap::Parser;
use cli::Cli;
use std::io::IsTerminal;

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

// Entry point: parse CLI → setup check → load config → apply CLI overrides →
// init debug → start spinner → run orchestrator → safety gate → print result
// → optional yolo exec.
// Safety details (pass/category/matched) are only shown when debug > 0.
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.list_tools {
        print_tool_levels();
        std::process::exit(0);
    }

    // --setup flag
    if cli.setup {
        if !std::io::stderr().is_terminal() {
            eprintln!("error: --setup requires an interactive terminal");
            std::process::exit(1);
        }
        let existing = setup::load_existing_config_if_present();
        if let Err(e) = setup::run_interactive(existing.as_ref()) {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // First-run detection: auto-enter setup if no config, interactive, and not quiet
    if !config::config_exists() && std::io::stderr().is_terminal() && !cli.quiet {
        let existing = setup::load_existing_config_if_present();
        if let Err(e) = setup::run_interactive(existing.as_ref()) {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Non-interactive first-run: print hint unless quiet
    if !config::config_exists() && !cli.quiet {
        eprintln!(
            "hint: no config file found at {}. run 'cmdify --setup' to configure.",
            config::default_config_file_path().display()
        );
    }

    if cli.prompt.is_empty() {
        Cli::parse_from(["cmdify", "--help"]);
        return;
    }

    let user_prompt = cli.user_prompt();

    let (config, env_sources) = match config::Config::from_env(cli.config.as_deref()) {
        Ok((c, s)) => (c, s),
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let (config, cli_sources) = app::apply_cli_overrides(&cli, config);

    debug::init(config.debug_level);

    for src in &env_sources {
        debug!("Config: {} = {} ({})", src.key, src.value, src.source);
    }
    for src in &cli_sources {
        debug!("Config: {} = {} ({})", src.key, src.value, src.source);
    }

    let spinner = cli.spinner.unwrap_or(config.spinner);

    let lg = logger::CmdifyLogger::new(&config.model_name, &config.provider_name);

    let spinner_handle = spinner::Spinner::start(spinner);
    let pause_handle = spinner_handle.pause_handle();

    let result = orchestrator::run(&user_prompt, &config, Some(&lg), Some(&pause_handle)).await;

    spinner_handle.stop();

    match result {
        Ok(content) => {
            debug!("Final response: {}", content);
            if let Err(block) = app::safety_gate(&content, config.allow_unsafe) {
                eprintln!("error: command blocked by safety check");
                if config.debug_level > 0 {
                    eprintln!(
                        "  pass {}: {} — matched: \"{}\"",
                        block.pass, block.category, block.matched_text
                    );
                }
                eprintln!("  rerun with --unsafe (-u) to allow unsafe commands");
                std::process::exit(1);
            }

            println!("{}", content);

            if config.yolo {
                if let Err(e) = app::execute_command(&content, &lg) {
                    eprintln!("{}", e);
                    std::process::exit(1);
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
