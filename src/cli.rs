use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "cmdify",
    about = "Turn natural language into shell commands with AI",
    version
)]
pub struct Cli {
    #[arg(short = 'q', long = "quiet", help = "Disable the ask_user tool")]
    pub quiet: bool,

    #[arg(short = 'b', long = "blind", help = "Disable the find_command tool")]
    pub blind: bool,

    #[arg(short = 'n', long = "no-tools", help = "Disable all tools", conflicts_with_all = ["quiet", "blind"])]
    pub no_tools: bool,

    #[arg(
        trailing_var_arg = true,
        help = "Natural language description of the command to generate"
    )]
    pub prompt: Vec<String>,
}

impl Cli {
    pub fn user_prompt(&self) -> String {
        self.prompt.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn parse_basic_prompt() {
        let cli = Cli::try_parse_from(["cmdify", "list", "all", "files"]).unwrap();
        assert_eq!(cli.user_prompt(), "list all files");
        assert!(!cli.quiet);
        assert!(!cli.blind);
        assert!(!cli.no_tools);
    }

    #[test]
    fn parse_quiet_flag() {
        let cli = Cli::try_parse_from(["cmdify", "-q", "find files"]).unwrap();
        assert!(cli.quiet);
    }

    #[test]
    fn parse_blind_flag() {
        let cli = Cli::try_parse_from(["cmdify", "-b", "find files"]).unwrap();
        assert!(cli.blind);
    }

    #[test]
    fn parse_no_tools_flag() {
        let cli = Cli::try_parse_from(["cmdify", "-n", "find files"]).unwrap();
        assert!(cli.no_tools);
    }

    #[test]
    fn parse_long_flags() {
        let cli = Cli::try_parse_from(["cmdify", "--quiet", "--blind", "test"]).unwrap();
        assert!(cli.quiet);
        assert!(cli.blind);
    }

    #[test]
    fn no_args_returns_no_prompt() {
        let cli = Cli::try_parse_from(["cmdify"]).unwrap();
        assert!(cli.prompt.is_empty());
        assert_eq!(cli.user_prompt(), "");
    }

    #[test]
    fn help_is_generated() {
        let mut cmd = Cli::command();
        let help = cmd.render_help().to_string();
        assert!(help.contains("cmdify"));
        assert!(help.contains("quiet"));
        assert!(help.contains("blind"));
        assert!(help.contains("no-tools"));
    }
}
