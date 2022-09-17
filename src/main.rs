use crate::cli::Cli;
use clap::Parser;

mod cli;
mod fs;

fn main() {
    Cli::parse();
}
