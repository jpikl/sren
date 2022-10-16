use crate::cli::Cli;
use crate::fs::transfer;
use crate::fs::TransferMode;
use crate::line::LineReader;
use crate::line::Separator;
use crate::path::PathReader;
use clap::Parser;
use std::io;

mod cli;
mod fs;
mod line;
mod path;

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
    let line_reader = LineReader::new(stdin, separator);
    let mut path_reader = PathReader::new(line_reader);

    while let Some((src, dst)) = path_reader.read()? {
        transfer(src, dst, mode)?;

        if cli.verbose {
            println!("{} -> {}", src.to_string_lossy(), dst.to_string_lossy());
        }
    }

    Ok(())
}
