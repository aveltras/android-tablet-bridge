mod cli;
mod parser;

use std::io::{self};

fn main() -> Result<(), io::Error> {
    cli::run()
}
