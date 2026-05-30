use clap_complete::{Shell, generate};

use crate::cli::{Cli, CompletionShell};
use crate::error::AppResult;

pub fn run_completions(shell: CompletionShell) -> AppResult<()> {
    let mut command = Cli::command();
    let name = command.get_name().to_string();
    let shell: Shell = shell.into();
    generate(shell, &mut command, &name, &mut std::io::stdout());
    Ok(())
}

impl From<CompletionShell> for Shell {
    fn from(value: CompletionShell) -> Self {
        match value {
            CompletionShell::Bash => Shell::Bash,
            CompletionShell::Elvish => Shell::Elvish,
            CompletionShell::Fish => Shell::Fish,
            CompletionShell::PowerShell => Shell::PowerShell,
            CompletionShell::Zsh => Shell::Zsh,
        }
    }
}
