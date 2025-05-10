use clap::Parser;
use std::env;
use std::process::{Command as StdCommand, ExitStatus, Output as ProcessOutput};

mod ai_explainer;
mod ai_utils;
mod cli;
mod config;
mod errors;

use crate::cli::{args_contain_help, CommitArgs, EnhancerSubCommand, GitEnhancerArgs};
use ai_explainer::{explain_git_command, explain_git_command_output};
use ai_utils::{OpenAIChatCompletionResponse, OpenAIChatRequest, ChatMessage}; 
use config::AppConfig;
use errors::{AppError, GitError, AIError}; 

struct CommandOutput {
    stdout: String,
    stderr: String,
    status: ExitStatus,
}

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
    let current_dir = env::current_dir()
        .map_err(|e| AppError::Io("Failed to get current directory".to_string(), e))?;
    if !current_dir.join(".git").exists() {
        tracing::error!("Error: Not a git repository (or any of the parent directories).");
        return Err(GitError::NotARepository.into());
    }

    let raw_cli_args: Vec<String> = std::env::args().skip(1).collect();
    let global_ai_idx = raw_cli_args.iter().position(|arg| arg == "--ai");

    if let Some(idx_of_ai) = global_ai_idx {
        tracing::debug!("Global --ai flag detected at index {}.", idx_of_ai);
        let mut effective_command_args: Vec<String> = raw_cli_args.clone();
        effective_command_args.remove(idx_of_ai);

        if args_contain_help(&effective_command_args) {
            tracing::info!("Global --ai with help flag detected. Explaining Git command output...");
            let cmd_output = execute_git_command_and_capture_output(&effective_command_args)?;
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
            let mut enhancer_args_to_parse = vec!["git-enhancer-dummy".to_string()];
            enhancer_args_to_parse.extend_from_slice(&effective_command_args);
            match GitEnhancerArgs::try_parse_from(&enhancer_args_to_parse) {
                Ok(parsed_enhancer_args) => match parsed_enhancer_args.command {
                    EnhancerSubCommand::Commit(commit_args) if commit_args.ai => {
                        tracing::info!("Global --ai followed by 'commit --ai'. Executing AI commit message generation...");
                        handle_commit(commit_args, &config).await?;
                    }
                    _ => {
                        tracing::info!("Global --ai detected for command: git {}", effective_command_args.join(" "));
                        match explain_git_command(&config, &effective_command_args).await {
                            Ok(explanation) => println!("{}", explanation),
                            Err(e) => return Err(AppError::AI(e)),
                        }
                    }
                },
                Err(_) => {
                    tracing::info!("Global --ai detected for command: git {}", effective_command_args.join(" "));
                    match explain_git_command(&config, &effective_command_args).await {
                        Ok(explanation) => println!("{}", explanation),
                        Err(e) => return Err(AppError::AI(e)),
                    }
                }
            }
        }
    } else {
        tracing::debug!("No global --ai flag detected.");
        let mut args_for_enhancer_parser = vec!["git-enhancer-dummy".to_string()];
        args_for_enhancer_parser.extend_from_slice(&raw_cli_args);
        match GitEnhancerArgs::try_parse_from(&args_for_enhancer_parser) {
            Ok(parsed_args) => match parsed_args.command {
                EnhancerSubCommand::Commit(args) => {
                    tracing::info!("Parsed as git-enhancer commit subcommand.");
                    handle_commit(args, &config).await?;
                }
            },
            Err(_) => {
                tracing::info!("Not recognized git-enhancer subcommand. Passing to git: git {}", raw_cli_args.join(" "));
                passthrough_to_git(&raw_cli_args)?;
            }
        }
    }
    Ok(())
}

fn execute_git_command_and_capture_output(args: &[String]) -> Result<CommandOutput, AppError> {
    let cmd_to_run = if args.is_empty() { vec!["--help".to_string()] } else { args.to_vec() };
    tracing::debug!("Capturing output: git {}", cmd_to_run.join(" "));
    let output = StdCommand::new("git")
        .args(&cmd_to_run)
        .output()
        .map_err(|e| AppError::Io(format!("Failed to execute: git {}", cmd_to_run.join(" ")), e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        tracing::warn!("Git cmd 'git {}' non-success {}. Stdout: [{}], Stderr: [{}]", cmd_to_run.join(" "), output.status, stdout, stderr);
    }
    Ok(CommandOutput { stdout, stderr, status: output.status })
}

fn passthrough_to_git(args: &[String]) -> Result<(), AppError> {
    let command_to_run = if args.is_empty() { vec!["--help".to_string()] } else { args.to_vec() };
    let cmd_str_log = command_to_run.join(" ");
    tracing::debug!("Passing to system git: git {}", cmd_str_log);
    let status = StdCommand::new("git")
        .args(&command_to_run)
        .status()
        .map_err(|e| AppError::Io(format!("Failed to execute system git: git {}", cmd_str_log), e))?;
    if !status.success() {
        tracing::warn!("Git passthrough 'git {}' failed: {}", cmd_str_log, status);
        return Err(AppError::Git(GitError::PassthroughFailed {
            command: format!("git {}", cmd_str_log),
            status_code: status.code(),
        }));
    }
    Ok(())
}

fn map_output_to_git_command_error(cmd_str: &str, output: ProcessOutput) -> GitError {
    GitError::CommandFailed {
        command: cmd_str.to_string(),
        status_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

async fn handle_commit_passthrough(args: CommitArgs, context_msg: String) -> Result<(), AppError> { 
    tracing::info!("Commit passthrough {}: msg: {:?}, args: {:?}", context_msg, args.message, args.passthrough_args);
    let mut cmd_builder = StdCommand::new("git");
    cmd_builder.arg("commit");
    if let Some(message) = &args.message {
        cmd_builder.arg("-m").arg(message);
    }
    for arg in &args.passthrough_args {
        cmd_builder.arg(arg);
    }
    let cmd_desc = format!("commit (passthrough {}) args: {:?}", context_msg, args.passthrough_args);
    let status = cmd_builder.status()
        .map_err(|e| AppError::Io(format!("Failed git {}", cmd_desc), e))?;
    if !status.success() {
        tracing::error!("Passthrough git {} failed with status {}", cmd_desc, status);
        return Err(AppError::Git(GitError::PassthroughFailed {
            command: format!("git {}", cmd_desc),
            status_code: status.code(),
        }));
    }
    tracing::info!("Passthrough git {} initiated/completed successfully.", cmd_desc);
    Ok(())
}

async fn handle_commit(args: CommitArgs, config: &AppConfig) -> Result<(), AppError> {
    if args.ai {
        tracing::info!("AI commit: Attempting to generate message...");
        let diff_out = StdCommand::new("git").arg("diff").arg("--staged").output()
            .map_err(|e| AppError::Git(GitError::DiffError(e)))?;
        if !diff_out.status.success() {
            tracing::error!("Error getting git diff. Is anything staged for commit?");
            return Err(map_output_to_git_command_error("git diff --staged", diff_out).into());
        }
        let diff = String::from_utf8_lossy(&diff_out.stdout);
        if diff.trim().is_empty() {
            tracing::info!("AI commit: No staged changes. Checking for --allow-empty.");
            if args.passthrough_args.contains(&"--allow-empty".to_string()) {
                let passthrough_commit_args = CommitArgs {
                     ai: false, 
                     message: None, 
                     passthrough_args: args.passthrough_args.clone(),
                 };
                return handle_commit_passthrough(passthrough_commit_args, "(AI commit with --allow-empty and no diff)".to_string()).await;
            } else {
                return Err(AppError::Git(GitError::NoStagedChanges));
            }
        }
        tracing::debug!("Staged changes for AI:\n{}", diff);
        let user_prompt = format!("Git diff:\n{}\nGenerate commit message.", diff.trim());
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: config.system_prompt.clone() },
            ChatMessage { role: "user".to_string(), content: user_prompt },
        ];
        let req_payload = OpenAIChatRequest { model: config.model_name.clone(), messages, temperature: Some(config.temperature), stream: false };
        if let Ok(json_str) = serde_json::to_string_pretty(&req_payload) { tracing::debug!("AI req:\n{}", json_str); }
        
        let client = reqwest::Client::new();
        let mut builder = client.post(&config.api_url);
        if let Some(key) = &config.api_key { builder = builder.bearer_auth(key); }
        let ai_resp = builder.json(&req_payload).send().await.map_err(AIError::RequestFailed)?;
        
        if !ai_resp.status().is_success() {
            let code = ai_resp.status();
            let body = ai_resp.text().await.unwrap_or_else(|_| "<no body>".into());
            tracing::error!("AI API request failed with status {}: {}", code, body);
            return Err(AppError::AI(AIError::ApiResponseError(code, body)));
        }
        let resp_data = ai_resp.json::<OpenAIChatCompletionResponse>().await.map_err(AIError::ResponseParseFailed)?;
        let ai_msg = resp_data.choices.get(0).map_or("", |c| &c.message.content);
        let final_msg = ai_utils::clean_ai_output(ai_msg).trim().to_string();

        if final_msg.is_empty() { 
            tracing::error!("AI returned an empty message.");
            return Err(AppError::AI(AIError::EmptyMessage)); 
        }
        tracing::info!("AI Message:\n---\n{}\n---", final_msg);

        let mut cmd_builder = StdCommand::new("git");
        cmd_builder.arg("commit").arg("-m").arg(&final_msg);
        for p_arg in &args.passthrough_args { cmd_builder.arg(p_arg); }
        let commit_out = cmd_builder.output().map_err(|e| AppError::Io("AI commit failed".into(), e))?;
        if !commit_out.status.success() {
            tracing::error!("Git commit command with AI message failed.");
            return Err(map_output_to_git_command_error("git commit -m <AI>", commit_out).into());
        }
        tracing::info!("Successfully committed with AI message.");
    } else {
        return handle_commit_passthrough(args, "(standard commit)".to_string()).await;
    }
    Ok(())
}