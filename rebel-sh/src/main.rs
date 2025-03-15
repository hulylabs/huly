// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::{ArgGroup, Parser};
use colored::*;
use rebel::core::{Module, VmValue};
use rebel::fs::fs_package;
use rebel::ssh::ssh_package;
use rustyline::{error::ReadlineError, DefaultEditor};
use std::io::{self, Read};

/// RebelDB interactive shell
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(group(
    ArgGroup::new("mode")
        .args(["execute", "file", "stdin"])
        .multiple(false)
        .required(false)
))]
struct Args {
    /// Execute the provided Rebel code and exit
    #[arg(short, long, value_name = "CODE")]
    execute: Option<String>,

    /// Read and execute Rebel code from the specified file
    #[arg(short, long, value_name = "FILE")]
    file: Option<String>,

    /// Read and execute Rebel code from standard input
    #[arg(short, long)]
    stdin: bool,

    /// Legacy mode: treat all arguments as code to execute
    #[arg(trailing_var_arg = true)]
    code: Vec<String>,
}

fn main() -> Result<()> {
    println!(
        "{} © 2025 Huly Labs • {}",
        "RebelDB™".bold(),
        "https://hulylabs.com".underline()
    );

    let mut module = Module::init(vec![0; 0x10000].into_boxed_slice())?;
    fs_package(&mut module)?;
    ssh_package(&mut module)?;

    // Parse command line arguments
    let args = Args::parse();

    // Handle --execute option
    if let Some(code) = args.execute {
        execute_command(&mut module, &code)?;
        return Ok(());
    }

    // Handle --file option
    if let Some(file_path) = args.file {
        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file '{}': {}", file_path, e))?;
        execute_command(&mut module, &content)?;
        return Ok(());
    }

    // Handle --stdin option
    if args.stdin {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        execute_command(&mut module, &buffer)?;
        return Ok(());
    }

    // Handle legacy mode (all arguments as code)
    if !args.code.is_empty() {
        let command = args.code.join(" ");
        execute_command(&mut module, &command)?;
        return Ok(());
    }

    // Interactive mode
    println!("Type {} or press Ctrl+D to exit\n", ":quit".red().bold());

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

                execute_command(&mut module, &line)?;
            }
            Err(ReadlineError::Interrupted) => {
                println!("Ctrl-C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Bye!");
                break;
            }
            Err(err) => {
                println!("{}", err);
                break;
            }
        }
    }

    // Save history
    // rl.save_history(&history_path)?;

    Ok(())
}

fn execute_command(module: &mut Module<Box<[u32]>>, command: &str) -> Result<()> {
    let result = module
        .parse(command)
        .and_then(|block| module.eval(block))
        .and_then(|result: VmValue| module.to_value(result));

    match result {
        Ok(value) => println!("{} {}", "OK:".green(), value),
        Err(e) => eprintln!("{} {}", "ERROR:".red().bold(), e),
    }

    Ok(())
}
