use clap::Parser;
use std::env;
use std::io::Write;
use std::process::Command;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This is a simple way to check if we are in a git repository.
    // A more robust check might involve `git rev-parse --is-inside-work-tree`.
    if !env::current_dir()?.join(".git").exists() {
        eprintln!("Error: Not a git repository (or any of the parent directories).");
        std::process::exit(1);
    }

    let cli = Cli::parse();

    Ok(())
}

fn handle_commit(args: CommitArgs) -> Result<(), Box<dyn std::error::Error>> {
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
        // let client = reqwest::Client::new();
        // let res = client.post("YOUR_AI_ENDPOINT")
        //                 .json(&serde_json::json!({"diff": diff}))
        //                 .send()
        //                 .await?
        //                 .json::<AiResponse>()
        // let ai_generated_message = res.commit_message;

        let ai_generated_message = if diff.trim().is_empty() {
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
        commit_command.arg("commint");
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
