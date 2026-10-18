use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "ccswitch",
    version,
    about = "Switch between isolated Claude Code profiles",
    after_help = "Examples:\n  ccswitch add work\n  ccswitch use work\n  ccswitch use client --here\n  eval \"$(ccswitch env work)\"\n  ccswitch status\n  ccswitch run -- --resume\n  ccswitch run personal -- -p \"hello\"\n  ccswitch code work -- ~/projects/app\n  ccswitch bypass personal on\n  ccswitch share work\n\n\
                  Environment:\n  CCSWITCH_HOME    Where profiles are stored (default: ~/.ccswitch)\n  CCSWITCH_PROFILE Profile to use in this shell, before .ccswitch files and the default\n  CCSWITCH_CLAUDE  Claude Code binary to run (default: claude)\n  CCSWITCH_CODE    VS Code binary to run (default: code)"
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
        /// Share CLAUDE.md, skills, agents and more from ~/.claude (see `ccswitch share`)
        #[arg(long)]
        share: bool,
    },
    /// List profiles (* marks the default)
    #[command(visible_alias = "ls")]
    List,
    /// Set the default profile, or the profile for this folder with --here
    Use {
        name: String,
        /// Write a .ccswitch file here instead, so this folder and its subfolders use the profile
        #[arg(long)]
        here: bool,
    },
    /// Print the profile in use here (stderr says why: default, .ccswitch or $CCSWITCH_PROFILE)
    Current,
    /// Show every profile's account, plan, MCP servers and last use
    #[command(visible_alias = "st")]
    Status {
        /// Print JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Print shell commands that select a profile for this shell: eval "$(ccswitch env work)"
    Env {
        #[arg(conflicts_with = "unset")]
        name: Option<String>,
        /// Shell syntax to print [default: guessed from $SHELL]
        #[arg(long, value_enum)]
        shell: Option<Shell>,
        /// Print commands that clear the selection instead
        #[arg(long)]
        unset: bool,
    },
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
    /// Share CLAUDE.md, skills, agents and more from ~/.claude into a profile
    Share(ShareArgs),
    /// Print a profile's config directory (or the default's)
    Path { name: Option<String> },
}

#[derive(Args)]
pub struct ShareArgs {
    /// Profile to share into
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub name: Option<String>,
    /// Apply to every profile
    #[arg(long)]
    pub all: bool,
    /// Folder to share from [default: $CCSWITCH_SHARE_FROM, else ~/.claude]
    #[arg(long, value_name = "DIR")]
    pub from: Option<std::path::PathBuf>,
    /// Items to share, comma separated. Default: CLAUDE.md, skills, agents, commands, plugins,
    /// hooks, output-styles, keybindings.json. Add settings.json to copy its hooks, enabledPlugins,
    /// env, statusLine, outputStyle and extraKnownMarketplaces keys (permissions stay per profile).
    #[arg(long, value_delimiter = ',', value_name = "ITEMS")]
    pub only: Vec<String>,
    /// Replace a profile's own copy, keeping it as <name>.bak
    #[arg(long, conflicts_with_all = ["undo", "status"])]
    pub force: bool,
    /// Remove the links instead of creating them
    #[arg(long, conflicts_with = "status")]
    pub undo: bool,
    /// Show what is shared, without changing anything
    #[arg(long)]
    pub status: bool,
    /// Show what would change, without changing anything
    #[arg(long, conflicts_with = "status")]
    pub dry_run: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum Shell {
    /// sh, bash, zsh and other POSIX shells
    #[value(alias = "bash", alias = "zsh")]
    Sh,
    Fish,
    #[value(alias = "pwsh")]
    Powershell,
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
