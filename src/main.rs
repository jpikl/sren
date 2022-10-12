use crate::cli::Cli;
use clap::Parser;

mod cli;
mod fs;
mod instr;
mod io;

fn main() {
    Cli::parse();
}
