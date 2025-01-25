// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use anyhow::Result;
use colored::*;
use rebel::eval::Process;
use rebel::value::Memory;
use rustyline::{error::ReadlineError, DefaultEditor};

fn main() -> Result<()> {
    println!(
        "{} © 2025 Huly Labs • {}",
        "RebelDB™".bold(),
        "https://hulylabs.com".underline()
    );
    println!("Type {} or press Ctrl+D to exit\n", ":quit".red().bold());

    let mut bytes = vec![0; 0x10000];
    let mut memory = Memory::new(&mut bytes, 0x1000, 0x1000)?;
    let mut process = Process::new(&mut memory);
    process.load_module(&rebel::boot::CORE_MODULE)?;

    let mut rl = DefaultEditor::new()?;

    // let history_path = PathBuf::from(".history");
    // if rl.load_history(&history_path).is_err() {
    //     println!("No previous history.");
    // }

    loop {
        let readline = rl.readline(&"RebelDB™ ❯ ".to_string());

        match readline {
            Ok(line) => {
                // Add to history
                rl.add_history_entry(line.as_str())?;

                // Handle special commands
                if line.trim() == ":quit" {
                    break;
                }

                match process.eval(&line) {
                    Ok(result) => println!("{}:  {:?}", "OK".green(), result),
                    Err(err) => eprintln!("{}: {}", "ERR".red().bold(), err),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Bye!");
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
