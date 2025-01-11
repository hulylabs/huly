// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// main.rs:

use anyhow::Result;
use colored::*;
use rustyline::{error::ReadlineError, DefaultEditor};

fn evaluate(line: &str) -> Result<String> {
    Ok(line.to_string())
}

fn main() -> Result<()> {
    println!(
        "{} © 2025 Huly Labs • {}",
        "RebelDB™".bold(),
        "https://hulylabs.com".underline()
    );
    println!("Type {} to exit", ":quit".red().bold());

    // Initialize interpreter
    // let mut interpreter = Interpreter::new();

    // Setup rustyline editor
    let mut rl = DefaultEditor::new()?;

    // Load history from previous sessions
    // let history_path = PathBuf::from(".history");
    // if rl.load_history(&history_path).is_err() {
    //     println!("No previous history.");
    // }

    // REPL loop
    loop {
        // Read
        let readline = rl.readline(&"RebelDB™ • ".to_string());

        match readline {
            Ok(line) => {
                // Add to history
                rl.add_history_entry(line.as_str())?;
                // Handle special commands
                if line.trim() == ":quit" {
                    break;
                }

                // Eval & Print
                // match evaluate(&mut interpreter, &line) {
                match evaluate(&line) {
                    Ok(result) => println!("{}", result.bright_green()),
                    Err(err) => eprintln!("{}: {}", "Error".red().bold(), err),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    // rl.save_history(&history_path)?;

    Ok(())
}
