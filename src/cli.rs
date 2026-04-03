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
        short = 'y',
        long = "yolo",
        help = "Execute the generated command after printing it"
    )]
    pub yolo: bool,

    #[arg(
        short = 's',
        long = "spinner",
        value_name = "N",
        help = "Spinner style: 1 (default bar), 2 (braille), or 3 (dots)"
    )]
    pub spinner: Option<u8>,

    #[arg(
        short = 'c',
        long = "config",
        value_name = "FILE",
        help = "Path to config file (must exist)"
    )]
    pub config: Option<std::path::PathBuf>,

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
    fn parse_yolo_flag() {
        let cli = Cli::try_parse_from(["cmdify", "-y", "find files"]).unwrap();
        assert!(cli.yolo);
        assert!(!cli.no_tools);
    }

    #[test]
    fn parse_yolo_long_flag() {
        let cli = Cli::try_parse_from(["cmdify", "--yolo", "find files"]).unwrap();
        assert!(cli.yolo);
    }

    #[test]
    fn parse_spinner_short() {
        let cli = Cli::try_parse_from(["cmdify", "-s", "2", "find files"]).unwrap();
        assert_eq!(cli.spinner, Some(2));
    }

    #[test]
    fn parse_spinner_long() {
        let cli = Cli::try_parse_from(["cmdify", "--spinner", "3", "find files"]).unwrap();
        assert_eq!(cli.spinner, Some(3));
    }

    #[test]
    fn spinner_default() {
        let cli = Cli::try_parse_from(["cmdify", "find files"]).unwrap();
        assert_eq!(cli.spinner, None);
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
        assert!(help.contains("yolo"));
        assert!(help.contains("spinner"));
        assert!(help.contains("config"));
    }

    #[test]
    fn parse_config_short() {
        let cli = Cli::try_parse_from(["cmdify", "-c", "/my/config.toml", "test"]).unwrap();
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/my/config.toml"))
        );
    }

    #[test]
    fn parse_config_long() {
        let cli = Cli::try_parse_from(["cmdify", "--config", "/my/config.toml", "test"]).unwrap();
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/my/config.toml"))
        );
    }

    #[test]
    fn config_default_none() {
        let cli = Cli::try_parse_from(["cmdify", "test"]).unwrap();
        assert!(cli.config.is_none());
    }
}
