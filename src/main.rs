use crate::cli::Cli;
use clap::Parser;

mod cli;

fn main() {
    Cli::parse();
}
