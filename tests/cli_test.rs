use clap::{CommandFactory, Parser};
use cmdify::cli::Cli;

#[test]
fn parse_with_prompt() {
    let cli = Cli::try_parse_from(["cmdify", "find", "all", "pdf", "files"]).unwrap();
    assert_eq!(cli.user_prompt(), "find all pdf files");
}

#[test]
fn parse_with_no_args() {
    let cli = Cli::try_parse_from(["cmdify"]).unwrap();
    assert!(cli.prompt.is_empty());
}

#[test]
fn parse_quiet_flag() {
    let cli = Cli::try_parse_from(["cmdify", "-q", "test"]).unwrap();
    assert!(cli.quiet);
}

#[test]
fn parse_blind_flag() {
    let cli = Cli::try_parse_from(["cmdify", "--blind", "test"]).unwrap();
    assert!(cli.blind);
}

#[test]
fn parse_no_tools_flag() {
    let cli = Cli::try_parse_from(["cmdify", "--no-tools", "test"]).unwrap();
    assert!(cli.no_tools);
}

#[test]
fn help_contains_usage() {
    let help = Cli::command().render_help().to_string();
    assert!(help.contains("cmdify"));
    assert!(help.contains("quiet"));
    assert!(help.contains("blind"));
}
