use clap::Parser;
use std::env;

mod ai_explainer;
mod ai_utils;
mod cli;
mod commit_commands;
mod config;
mod errors;
mod git_commands;
mod types;

// CLI and core types
use crate::cli::{args_contain_help, CommitArgs, EnhancerSubCommand, GitEnhancerArgs};

/// Checks if the `--ai` flag is present in the provided arguments
fn args_contain_ai(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--ai")
}
use crate::git_commands::{execute_git_command_and_capture_output, passthrough_to_git, map_output_to_git_command_error, is_git_available, is_in_git_repository};
use crate::commit_commands::{handle_commit, handle_commit_passthrough};
use config::AppConfig;
use errors::{AppError, GitError, AIError};

// External dependencies
use ai_explainer::{explain_git_command, explain_git_command_output};
use ai_utils::{OpenAIChatCompletionResponse, OpenAIChatRequest, ChatMessage}; 



fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_app());

    if let Err(e) = result {
        tracing::error!("Application failed: {}", e);
        let exit_code = match e {
            AppError::Git(GitError::PassthroughFailed { status_code, .. }) => {
                status_code.unwrap_or(128) 
            }
            AppError::Git(GitError::CommandFailed { status_code, .. }) => {
                status_code.unwrap_or(128)
            }
            _ => 1, 
        };
        std::process::exit(exit_code);
    }
}

async fn run_app() -> Result<(), AppError> {
    let config = AppConfig::load()?;
    // First check if git is available
    if !is_git_available()? {
        tracing::error!("Error: Git is not available on this system.");
        return Err(AppError::Io("Git command not found or not executable".to_string(), std::io::Error::new(std::io::ErrorKind::NotFound, "Git not available")));
    }
    
    // Then check if we're in a git repository
    if !is_in_git_repository()? {
        tracing::error!("Error: Not a git repository (or any of the parent directories).");
        return Err(GitError::NotARepository.into());
    }

    let raw_cli_args: Vec<String> = std::env::args().skip(1).collect();
    // 1. Check for help flags first
    if args_contain_help(&raw_cli_args) {
        let ai_flag_present = args_contain_ai(&raw_cli_args);
        if ai_flag_present {
            tracing::info!("Help flag detected with --ai. Explaining Git command output...");
            let mut command_to_execute_for_help = raw_cli_args.clone();
            command_to_execute_for_help.retain(|arg| arg != "--ai");

            // After removing the --ai flag:
            // - For `git-enhancer --ai --help` -> `--help` remains in the command
            // - For `git-enhancer --ai commit --help` -> `commit --help` remains
            // The help flags (-h/--help) are NOT removed by the retain operation,
            // only the --ai flag is removed
            // Since help flags always remain, we'll never have an empty command
            let cmd_output = execute_git_command_and_capture_output(&command_to_execute_for_help)?;
            let mut text_to_explain = cmd_output.stdout;
            if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
                text_to_explain.push_str("\n--- Stderr ---\n");
                text_to_explain.push_str(&cmd_output.stderr);
            }
            match explain_git_command_output(&config, &text_to_explain).await {
                Ok(explanation) => println!("{}", explanation),
                Err(e) => return Err(AppError::AI(e)),
            }
        } else {
            // No --ai, just passthrough the help request to git
            tracing::info!("Help flag detected without --ai. Passing to git.");
            passthrough_to_git(&raw_cli_args)?;
        }
    } else {
        // 2. Not a help request, try parsing as git-enhancer subcommand or global AI explanation
        let mut enhancer_parser_args = vec!["git-enhancer-dummy".to_string()]; // Dummy executable name for clap
        enhancer_parser_args.extend_from_slice(&raw_cli_args);

        match GitEnhancerArgs::try_parse_from(&enhancer_parser_args) {
            Ok(parsed_enhancer_args) => {
                // Successfully parsed as a git-enhancer specific command
                match parsed_enhancer_args.command {
                    EnhancerSubCommand::Commit(commit_args) => {
                        // This handles `git-enhauser commit --ai` as well as `git-enhauser commit -m "message"`
                        // The `handle_commit` function itself checks `commit_args.ai`
                        tracing::info!("Parsed as git-enhancer commit subcommand. Delegating to handle_commit.");
                        handle_commit(commit_args, &config).await?;
                    }
                    // Future: Add other EnhancerSubCommand arms here if they are added to cli.rs
                }
            }
            Err(_) => {
                // Failed to parse as a specific git-enhancer subcommand.
                // This could be a global --ai explanation request for a generic git command (e.g. `git-enhauser --ai status`),
                // or just a command to passthrough (e.g. `git-enhauser status`).
                let ai_flag_present = args_contain_ai(&raw_cli_args);
                if ai_flag_present {
                    tracing::info!("Not a specific git-enhancer subcommand, but --ai flag detected. Explaining Git command...");
                    let mut command_to_explain = raw_cli_args.clone();
                    command_to_explain.retain(|arg| arg != "--ai"); // Remove all occurrences of --ai

                    if command_to_explain.is_empty() {
                        // Handle `git-enhauser --ai` (with no actual command after removing --ai)
                        // Default to explaining "git --help"
                        tracing::debug!("No specific command with global --ai, explaining 'git --help'.");
                        command_to_explain.push("--help".to_string());
                    }
                    match explain_git_command(&config, &command_to_explain).await {
                        Ok(explanation) => println!("{}", explanation),
                        Err(e) => return Err(AppError::AI(e)),
                    }
                } else {
                    // No --ai, not a known enhancer subcommand. Pass through to git.
                    // e.g., `git-enhauser status`
                    tracing::info!("Not a recognized git-enhancer subcommand and no --ai. Passing to git.");
                    passthrough_to_git(&raw_cli_args)?;
                }
            }
        }
    }
    Ok(())
}






