// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use anyhow::Result;
use colored::*;
use rebel::boot::CORE_MODULE;
use rebel::core::{init_memory, EvalContext};
use rebel::parse::Parser;
use rustyline::{error::ReadlineError, DefaultEditor};

fn main() -> Result<()> {
    println!(
        "{} © 2025 Huly Labs • {}",
        "RebelDB™".bold(),
        "https://hulylabs.com".underline()
    );
    println!("Type {} or press Ctrl+D to exit\n", ":quit".red().bold());

    let mut buf = vec![0; 0x10000].into_boxed_slice();
    let mut mem = init_memory(&mut buf, 256, 1024)?;
    mem.load_module(&CORE_MODULE)?;
    let mut ctx = EvalContext::new(&mut mem);

    let mut rl = DefaultEditor::new()?;

    // let history_path = PathBuf::from(".history");
    // if rl.load_history(&history_path).is_err() {
    //     println!("No previous history.");
    // }

    loop {
        let readline = rl.readline(&"RebelDB™ ❯ ".to_string());

        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;
                if line.trim() == ":quit" {
                    break;
                }
                let mut parser = Parser::new(&line, &mut ctx);
                match parser.parse() {
                    Ok(_) => match ctx.eval_parsed() {
                        Ok(_) => {
                            let result = ctx.pop_stack()?.collect::<Vec<_>>();
                            if result.is_empty() {
                                println!("{}: {:?}", "OK".green(), "None")
                            } else {
                                println!("{}: {:?}", "OK".green(), result[0])
                            }
                        }
                        Err(err) => eprintln!("{}: {}", "ERR".red().bold(), err),
                    },
                    Err(err) => eprintln!("{}: {}", "SYNTAX ERR".cyan().bold(), err),
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
