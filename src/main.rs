use crate::cli::Cli;
use clap::Parser;

mod cli;
mod fs;
mod input;

fn main() {
    Cli::parse();
}
