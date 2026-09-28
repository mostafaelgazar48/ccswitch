use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "ccswitch",
    version,
    about = "Switch between isolated Claude Code profiles",
    after_help = "Examples:\n  ccswitch add work\n  ccswitch use work\n  ccswitch run -- --resume\n  ccswitch run personal -- -p \"hello\"\n  ccswitch code work -- ~/projects/app\n  ccswitch bypass personal on\n\n\
                  Environment:\n  CCSWITCH_HOME    Where profiles are stored (default: ~/.ccswitch)\n  CCSWITCH_CLAUDE  Claude Code binary to run (default: claude)\n  CCSWITCH_CODE    VS Code binary to run (default: code)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a profile and start Claude so you can log in
    Add {
        name: String,
        /// Create the profile without starting Claude
        #[arg(long)]
        no_login: bool,
        /// Make it the default profile (automatic when there is no default yet)
        #[arg(long)]
        default: bool,
    },
    /// List profiles (* marks the default)
    #[command(visible_alias = "ls")]
    List,
    /// Set the default profile
    Use { name: String },
    /// Print the default profile
    Current,
    /// Launch Claude with a profile (or the default); put Claude's own flags after --
    Run {
        name: Option<String>,
        /// Skip all permission prompts for this session (--dangerously-skip-permissions)
        #[arg(long)]
        bypass: bool,
        /// Arguments passed to Claude unchanged
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Open VS Code with a profile, so the Claude Code extension uses it
    Code {
        name: Option<String>,
        /// Arguments passed to VS Code unchanged (default: the current folder)
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Rename a profile (Claude may ask you to log in again)
    #[command(visible_alias = "mv")]
    Rename { old: String, new: String },
    /// Delete a profile and its saved login
    #[command(visible_alias = "rm")]
    Remove {
        name: String,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Show or change whether a profile skips all permission prompts
    Bypass {
        name: String,
        /// Omit to show the current state
        state: Option<Toggle>,
    },
    /// Print a profile's config directory (or the default's)
    Path { name: Option<String> },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Toggle {
    On,
    Off,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn run_accepts_trailing_args_without_name() {
        let cli = Cli::try_parse_from(["ccswitch", "run", "--", "--resume"]).unwrap();
        match cli.command {
            Command::Run { name, args, .. } => {
                assert_eq!(name, None);
                assert_eq!(args, ["--resume"]);
            }
            _ => panic!("expected run"),
        }
    }
}
