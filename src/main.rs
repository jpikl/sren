use crate::cli::Cli;
use crate::fs::transfer;
use crate::fs::TransferMode;
use crate::line::LineReader;
use crate::line::Separator;
use crate::path::PathReader;
use crate::verbose::Logger;
use atty::Stream;
use clap::Parser;
use std::error::Error;
use std::io;
use std::process;
use termcolor::ColorChoice;
use termcolor::StandardStream;

mod cli;
mod fs;
mod line;
mod path;
mod verbose;

fn main() {
    if let Err(error) = try_main() {
        eprintln!("error: {}", error);
        process::exit(1);
    }
}

fn try_main() -> Result<(), Box<dyn Error>> {
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

    let colors = if atty::is(Stream::Stdout) {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };

    let stdin = io::stdin();
    let stdout = StandardStream::stdout(colors);

    let line_reader = LineReader::new(stdin.lock(), separator);
    let mut path_reader = PathReader::new(line_reader);
    let mut logger = Logger::new(stdout.lock());

    while let Some((src, dst)) = path_reader.read()? {
        if cli.verbose {
            logger.begin(src, dst, mode)?;
        }

        match transfer(src, dst, mode) {
            Ok(()) => {
                if cli.verbose {
                    logger.success()?;
                }
            }
            Err(error) => {
                if cli.verbose {
                    logger.failure()?;
                }
                return Err(error.into());
            }
        }
    }

    Ok(())
}
