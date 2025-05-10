use clap::Parser;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

// 定义发送到Ollama /v1/chat/completions端点的请求体结构体
#[derive(Serialize, Debug)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>, // OpenAI API通常将temperature作为可选的顶层参数
    stream: bool,
    // 你可以在这里添加其他OpenAI支持的选项，例如 top_p, max_tokens 等
    // "max_tokens": Option<u32>,
    // "top_p": Option<f32>,
}

#[derive(Deserialize, Debug, Clone)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct OpenAIChoice {
    index: u32,
    message: OpenAIMessage,
    finish_reason: String,
    // logprobs: Option<serde_json::Value>, // 如果需要解析logprobs
}

#[derive(Deserialize, Debug)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Deserialize, Debug)]
struct OpenAIChatCompletionResponse {
    id: String,
    object: String,
    created: i64, // 通常是Unix时间戳
    model: String,
    system_fingerprint: Option<String>, // 根据您的示例，这个字段存在
    choices: Vec<OpenAIChoice>,
    usage: OpenAIUsage,
}

#[derive(Parser, Debug)]
#[clap(author="Huchen", version="0.1.0", about="Enhances Git with AI support", long_about=None)]
struct Cli {
    #[clap(subcommand)]
    command: GitCommands,
}

#[derive(Parser, Debug)]
enum GitCommands {
    /// Handle git commit operation
    Commit(CommitArgs), // Future: Add(AddArgs)
                        // Future: Config(ConfigArgs)
}

#[derive(Parser, Debug)]
struct CommitArgs {
    /// Use AI to generate the commit message
    #[clap(long)]
    ai: bool,

    /// Pass a message to the commit
    #[clap(short, long)]
    message: Option<String>,

    /// Allow all other flags and arguments to be passed through to git commit
    #[clap(allow_hyphen_values = true, last = true)]
    passthrough_args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This is a simple way to check if we are in a git repository.
    // A more robust check might involve `git rev-parse --is-inside-work-tree`.
    if !env::current_dir()?.join(".git").exists() {
        eprintln!("Error: Not a git repository (or any of the parent directories).");
        std::process::exit(1);
    }

    let cli = Cli::parse();
    match cli.command {
        GitCommands::Commit(args) => {
            handle_commit(args).await?;
        }
    }

    Ok(())
}

async fn handle_commit(args: CommitArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.ai {
        println!("AI flag detected. Attempting to generate commit message...");

        // 1. Get staged changes using `git diff --staged`
        let diff_output = Command::new("git").arg("diff").arg("--staged").output()?;

        if !diff_output.status.success() {
            eprintln!("Error getting git diff. Is anything staged for commit?");
            std::io::stderr().write_all(&diff_output.stderr)?;
            std::process::exit(diff_output.status.code().unwrap_or(1));
        }

        let diff = String::from_utf8_lossy(&diff_output.stdout);

        if diff.trim().is_empty() {
            println!("No changes staged for commit. Nothing for AI to process.");
            // Optionally, proceed with a normal commit if other args are present,
            // or exit. For now, we'll just inform and let git handle it if it proceeds.
        } else {
            println!("Staged changes:\n{}", diff);
        }

        // 2.Send diff to AI (Simulated for now)
        // In a real scenario, you would make an HTTP request here.
        // For example, using `reqwest`
        let openai_api_url = "http://localhost:11434/v1/chat/completions";
        let model_name = "qwen3:32b-q8_0";
        let system_prompt = r#"
**系统提示（System Prompt）**:

你是一个智能助手，能够根据给定的代码变更生成清晰、简洁的 Git commit 信息。请根据以下信息生成适当的 commit 信息：

**输入信息**:
-  Git diff 信息（显示哪些文件被修改、添加或删除，以及具体的代码变化）

**要求**:
1. 提供简洁明了的描述，概括主要的变更内容。
2. 使用动词开头，描述变更的目的（例如：修复、添加、更新、删除）。
3. 如果适用，包含相关的上下文或背景信息。
4. 避免使用技术术语，确保描述易于理解。

**示例输入**:
```
diff --git a/example.py b/example.py
index 83db48f..2c6f1f0 100644
--- a/example.py
+++ b/example.py
@@ -1,5 +1,5 @@
 def add(a, b):
-     return a + b
+    return a + b + 1  # 增加了1以满足新的需求
```

**示例输出**:
```
更新 add 函数以满足新的需求，返回值增加1。
```
"#;
        let user_prompt = format!(
            r#"
这里是 git diff 信息：
{}
请帮我生成合适的 commit 信息
"#,
            diff.trim()
        );
        println!("diff info: {}", diff.trim());
        println!("user prompt: {}", user_prompt);
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
        let temperature = Some(0.7);

        // 构建请求体
        let request_payload = OpenAIChatRequest {
            model: model_name.to_string(),
            messages,
            temperature,
            stream: false, // 设置 false 获取完整响应，而不是流式响应
        };

        // 打印将要发送的JSON payload (可选，用于调试)
        match serde_json::to_string_pretty(&request_payload) {
            Ok(json_string) => println!("Sending JSON payload:\n{}", json_string),
            Err(e) => eprintln!("Error serializing request: {}", e),
        }

        let client = reqwest::Client::new();
        let openai_response = client
            .post(openai_api_url)
            .json(&request_payload) // reqwest 会自动设置 Content-Type: application/json
            .send()
            .await?;

        let mut ai_generated_message = "".to_string();
        if openai_response.status().is_success() {
            match openai_response.json::<OpenAIChatCompletionResponse>().await {
                Ok(response) => {
                    if let Some(fp) = response.system_fingerprint {
                        println!("System Fingerprint: {}", fp);
                    }
                    if let Some(choice) = response.choices.get(0) {
                        println!("Finish Reason: {}", choice.finish_reason);
                        println!(
                            "Assistant's Reply (Role: {}):\n{}",
                            choice.message.role, choice.message.content
                        );
                        ai_generated_message.push_str(&choice.message.content);
                    } else {
                        println!("No choices found in response.");
                    }
                    println!("\nUsage:");
                    println!("  Prompt Tokens: {}", response.usage.prompt_tokens);
                    println!("  Completion Tokens: {}", response.usage.completion_tokens);
                    println!("  Total Tokens: {}", response.usage.total_tokens);
                }
                Err(e) => {
                    eprintln!("Failed to parse JSON response: {}", e);
                    // 如果解析失败，尝试打印原始文本以帮助调试
                    // let raw_text = response.text().await.unwrap_or_else(|_| "Failed to get raw text".to_string());
                    // eprintln!("Raw response text: {}", raw_text);
                }
            }
        }

        if ai_generated_message.trim().is_empty() {
            "chore: no changes detected by AI".to_string()
        } else {
            // Simulate AI processing
            format!(
                "AI Generated: feat: Implement feature based on changes\n\nDetails:\n{}",
                summarize_diff(&diff)
            )
        };

        println!(
            "AI Generated: Message: \n---\n{}\n---",
            ai_generated_message
        );

        // 3.Execute git commit with the AI-generated message
        let mut commit_command = Command::new("git");
        commit_command.arg("commit");
        commit_command.arg("-m");
        commit_command.arg(&ai_generated_message);

        // Add any passthrough arguments
        for arg in args.passthrough_args {
            commit_command.arg(arg);
        }

        let commit_status = commit_command.status()?;

        if !commit_status.success() {
            eprintln!("Git commit command failed.");
            std::process::exit(commit_status.code().unwrap_or(1));
        } else {
            println!("Successfully committed with AI-generated message.");
        }
    } else if let Some(message) = args.message {
        // Standard commit with user-provided message
        println!("Standard commit with message: {}", message);
        let mut commit_command = Command::new("git");
        commit_command.arg("commit");
        commit_command.arg("-m");
        commit_command.arg(&message);
        for arg in args.passthrough_args {
            commit_command.arg(arg);
        }
        let commit_status = commit_command.status()?;
        if !commit_status.success() {
            eprintln!("Git commit command failed.");
            std::process::exit(commit_status.code().unwrap_or(1));
        } else {
            println!("Successfully committed with provided message.");
        }
    } else {
        // Standard commit, let git oopen an editor or fail if no message
        println!("Standard commit (no message provided to enhancer, forwarding to git)...");
        let mut commit_command = Command::new("git");
        commit_command.arg("commit");
        for arg in args.passthrough_args {
            commit_command.arg(arg);
        }
        let commit_status = commit_command.status()?;
        if !commit_status.success() {
            eprintln!("Git commit command failed.");
            std::process::exit(commit_status.code().unwrap_or(1));
        } else {
            println!("Git commit process initiated.");
        }
    }

    Ok(())
}

// A very simple diff summarizer for simulation
fn summarize_diff(diff: &str) -> String {
    let lines: Vec<&str> = diff.lines().collect();
    let mut summary = String::new();
    let mut additions = 0;
    let mut deletions = 0;

    for line in lines {
        if line.starts_with("+") && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with("-") && !line.starts_with("---") {
            deletions += 1;
        }
    }

    if additions > 0 {
        summary.push_str(&format!("Added ~{} lines. ", additions));
    }

    if deletions > 0 {
        summary.push_str(&format!("Removed ~{} lines. ", deletions));
    }

    if summary.is_empty() {
        "No significant changes detected in diff summary.".to_string()
    } else {
        summary.trim().to_string()
    }
}
