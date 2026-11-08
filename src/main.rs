use claude_panorama::cli::{execute_non_gui, parse_cli_args, Command};
use claude_panorama::gui::run_gui;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_cli_args(&args) {
        Ok(Command::Gui) => {
            let code = run_gui();
            if code == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(code as u8)
            }
        }
        Ok(cmd) => match execute_non_gui(cmd) {
            Ok(output) => {
                if !output.ends_with('\n') {
                    println!("{output}");
                } else {
                    print!("{output}");
                }
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("erro: {err}");
                ExitCode::FAILURE
            }
        },
        Err(err) => {
            eprintln!("erro: {err}");
            ExitCode::from(2)
        }
    }
}
