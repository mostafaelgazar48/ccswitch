mod app;
mod cli;
mod error;
mod launch;
mod settings;
mod store;

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
