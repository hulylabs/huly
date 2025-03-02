// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use anyhow::Result;
use colored::*;
use rebel::core::Module;
use rustyline::{error::ReadlineError, DefaultEditor};

fn main() -> Result<()> {
    println!(
        "{} © 2025 Huly Labs • {}",
        "RebelDB™".bold(),
        "https://hulylabs.com".underline()
    );
    println!("Type {} or press Ctrl+D to exit\n", ":quit".red().bold());

    let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;

    let mut rl = DefaultEditor::new()?;

    // let history_path = PathBuf::from(".history");
    // if rl.load_history(&history_path).is_err() {
    //     println!("No previous history.");
    // }

    loop {
        let readline = rl.readline(&"RebelDB™ ❯ ");

        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;
                if line.trim() == ":quit" {
                    break;
                }

                match module.parse(line.as_str()) {
                    Ok(block) => match module.eval(block) {
                        Ok(result) => {
                            println!("{}: {:?}", "OK".green(), result)
                        }
                        Err(e) => eprintln!("{} {}", "ERROR:".red().bold(), e),
                    },
                    Err(err) => eprintln!("{}: {}", "PARSE:".cyan().bold(), err),
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
