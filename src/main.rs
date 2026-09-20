mod cli;
mod contributors;
mod git;
mod metadata;
mod navigation;
mod okf;
mod tree;
mod workspace;

use clap::Parser;

fn main() {
    if let Err(error) = cli::Cli::parse().run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
