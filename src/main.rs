mod app;
mod claude_json;
mod cli;
mod date;
mod error;
mod launch;
mod pricing;
mod sessions;
mod settings;
mod share;
mod shell;
mod store;
mod usage;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    let code = match app::run(cli.command) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ccswitch: {e}");
            1
        }
    };
    std::process::exit(code);
}
