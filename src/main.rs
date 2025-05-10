use clap::Parser;
use std::env;
use std::process::Command;

mod ai_utils;
mod cli;
mod config;
mod errors;

use ai_utils::{ChatMessage, OpenAIChatCompletionResponse, OpenAIChatRequest};
use cli::{Cli, CommitArgs, GitCommands};
use config::AppConfig;
use errors::{AppError, GitError, AIError, map_command_error};

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // Explicitly set stderr
        .init();
    if let Err(e) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_app())
    {
        tracing::error!("Application failed: {}", e);
        // Consider more specific exit codes based on error type
        std::process::exit(1);
    }
}

async fn run_app() -> Result<(), AppError> {
    let config = AppConfig::load()?;

    // This is a simple way to check if we are in a git repository.
    let current_dir = env::current_dir().map_err(|e| AppError::Io("Failed to get current directory".to_string(), e))?;
    if !current_dir.join(".git").exists() {
        tracing::error!("Error: Not a git repository (or any of the parent directories).");
        return Err(GitError::NotARepository.into());
    }

    let cli = Cli::parse();
    match cli.command {
        GitCommands::Commit(args) => {
            handle_commit(args, &config).await?;
        }
    }

    Ok(())
}

async fn handle_commit(args: CommitArgs, config: &AppConfig) -> Result<(), AppError> {
    if args.ai {
        tracing::info!("AI flag detected. Attempting to generate commit message...");

        // 1. Get staged changes using `git diff --staged`
        let diff_cmd_output = Command::new("git")
            .arg("diff")
            .arg("--staged")
            .output()
            .map_err(|e| GitError::DiffError(e))?;

        if !diff_cmd_output.status.success() {
            let err_msg = format!("Error getting git diff. Is anything staged for commit?");
            tracing::error!("{}", err_msg);
            // Log stderr from the command as well
            let stderr_output = String::from_utf8_lossy(&diff_cmd_output.stderr);
            tracing::debug!("git diff stderr: {}", stderr_output);
            let status = diff_cmd_output.status;
            return Err(map_command_error("git diff --staged", diff_cmd_output, status).into());
        }

        let diff = String::from_utf8_lossy(&diff_cmd_output.stdout);

        if diff.trim().is_empty() {
            tracing::info!("No changes staged for commit. Nothing for AI to process.");
            // Optionally, proceed with a normal commit if other args are present,
            // or exit. For now, we'll just inform and let git handle it if it proceeds.
        } else {
            tracing::debug!("Staged changes:\\n{}", diff);
        }

        // 2.Send diff to AI
        let openai_api_url = &config.api_url;
        let model_name = &config.model_name;
        let system_prompt = &config.system_prompt;

        let user_prompt = format!(
            r#"
这里是 git diff 信息：
{}
请帮我生成合适的 commit 信息
"#,
            diff.trim()
        );
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ];

        // 构建请求选项
        let temperature = Some(config.temperature);

        // 构建请求体
        let request_payload = OpenAIChatRequest {
            model: model_name.to_string(),
            messages,
            temperature,
            stream: false, // 设置 false 获取完整响应，而不是流式响应
        };

        // 打印将要发送的JSON payload (可选，用于调试)
        match serde_json::to_string_pretty(&request_payload) {
            Ok(json_string) => tracing::debug!("Sending JSON payload:\\n{}", json_string),
            Err(e) => tracing::warn!("Error serializing request: {}", e),
        }

        let client = reqwest::Client::new();
        let openai_response = client
            .post(openai_api_url)
            .json(&request_payload) // reqwest 会自动设置 Content-Type: application/json
            .send()
            .await
            .map_err(AIError::RequestFailed)?;

        let mut ai_generated_message = "".to_string();

        if !openai_response.status().is_success() {
            let status = openai_response.status();
            let body = openai_response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
            tracing::error!("AI API request failed with status {}: {}", status, body);
            return Err(AIError::ApiResponseError(status, body).into());
        }

        // Now that we know status is success, try to parse
        match openai_response.json::<OpenAIChatCompletionResponse>().await {
            Ok(response) => {
                if let Some(choice) = response.choices.get(0) {
                    ai_generated_message.push_str(&choice.message.content);
                } else {
                    tracing::warn!("No choices found in AI response.");
                    return Err(AIError::NoChoiceInResponse.into());
                }
            }
            Err(e) => {
                tracing::error!("Failed to parse successful JSON response from AI: {}", e);
                // It's possible the body was already consumed by .json(), or if not, log it here
                // let raw_text = openai_response.text().await.unwrap_or_else(|_| "Failed to get raw text".to_string());
                // tracing::debug!("Raw AI response text: {}", raw_text);
                return Err(AIError::ResponseParseFailed(e).into());
            }
        }

        let final_commit_message = if ai_generated_message.trim().is_empty() {
            tracing::warn!("AI returned an empty message.");
            return Err(AIError::EmptyMessage.into());
        } else {
            // The old placeholder logic:
            // format!(
            //     "AI Generated: feat: Implement feature based on changes\\n\\nDetails:\\n{}",
            //     summarize_diff(&diff)
            // )
            // Now we directly use ai_generated_message
            ai_generated_message.trim().to_string()
        };

        tracing::info!(
            "AI Generated Message: \\n---\\n{}\\n---",
            final_commit_message
        );

        // 3.Execute git commit with the AI-generated message
        let mut commit_command_builder = Command::new("git");
        commit_command_builder.arg("commit");
        commit_command_builder.arg("-m");
        commit_command_builder.arg(&final_commit_message); // Use the processed message

        // Add any passthrough arguments
        for arg in &args.passthrough_args { // Iterate over reference
            commit_command_builder.arg(arg);
        }
        
        let commit_output = commit_command_builder.output().map_err(|e| AppError::Io("Failed to execute git commit".to_string(), e))?;

        if !commit_output.status.success() {
            tracing::error!("Git commit command failed.");
            let status = commit_output.status;
            return Err(map_command_error(&format!("git commit -m \\\"{}\\\" {}", final_commit_message, args.passthrough_args.join(" ")), commit_output, status).into());
        } else {
            tracing::info!("Successfully committed with AI-generated message.");
        }
    } else if let Some(message) = args.message {
        // Standard commit with user-provided message
        tracing::info!("Standard commit with message: {}", message);
        let mut commit_command_builder = Command::new("git");
        commit_command_builder.arg("commit");
        commit_command_builder.arg("-m");
        commit_command_builder.arg(&message);
        for arg in &args.passthrough_args {
            commit_command_builder.arg(arg);
        }
        let commit_output = commit_command_builder.output().map_err(|e| AppError::Io("Failed to execute git commit".to_string(), e))?;
        if !commit_output.status.success() {
            tracing::error!("Git commit command failed.");
            let status = commit_output.status;
            return Err(map_command_error(&format!("git commit -m \\\"{}\\\" {}", message, args.passthrough_args.join(" ")), commit_output, status).into());
        } else {
            tracing::info!("Successfully committed with provided message.");
        }
    } else {
        // Standard commit, let git open an editor or fail if no message
        tracing::info!("Standard commit (no message provided to enhancer, forwarding to git)...");
        let mut commit_command_builder = Command::new("git");
        commit_command_builder.arg("commit");
        for arg in &args.passthrough_args {
            commit_command_builder.arg(arg);
        }
        let commit_output = commit_command_builder.output().map_err(|e| AppError::Io("Failed to execute git commit".to_string(), e))?;
        if !commit_output.status.success() {
            tracing::error!("Git commit command failed.");
            let status = commit_output.status;
            return Err(map_command_error(&format!("git commit {}", args.passthrough_args.join(" ")), commit_output, status).into());
        } else {
            tracing::info!("Git commit process initiated.");
        }
    }

    Ok(())
}
