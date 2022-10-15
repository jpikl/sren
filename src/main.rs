use crate::cli::Cli;
use crate::fs::transfer;
use crate::fs::TransferMode;
use crate::input::Separator;
use clap::Parser;
use std::io;

mod cli;
mod fs;
mod input;
mod instr;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let separator = if cli.null {
        Separator::Null
    } else {
        Separator::Newline
    };

    let mode = if cli.copy {
        TransferMode::Copy
    } else {
        TransferMode::Move
    };

    let stdin = io::stdin().lock();
    let input_reader = input::Reader::new(stdin, separator);
    let mut instr_reader = instr::Reader::new(input_reader);

    while let Some((src, dst)) = instr_reader.read()? {
        transfer(src, dst, mode)?;

        if cli.verbose {
            println!("{} -> {}", src.to_string_lossy(), dst.to_string_lossy());
        }
    }

    Ok(())
}
