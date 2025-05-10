use std::io;

// General Application Error
#[derive(Debug)]
pub enum AppError {
    Config(ConfigError),
    Git(GitError),
    AI(AIError),
    Io(String, io::Error), // For general I/O errors not covered by specific types
    // Add other top-level error categories as needed
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Config(e) => write!(f, "Configuration error: {}", e),
            AppError::Git(e) => write!(f, "Git command error: {}", e),
            AppError::AI(e) => write!(f, "AI interaction error: {}", e),
            AppError::Io(context, e) => write!(f, "I/O error while {}: {}", context, e),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Config(e) => Some(e),
            AppError::Git(e) => Some(e),
            AppError::AI(e) => Some(e),
            AppError::Io(_, e) => Some(e),
        }
    }
}

// Configuration Errors (moved from config.rs)
#[derive(Debug)]
pub enum ConfigError {
    FileRead(String, io::Error),
    JsonParse(String, serde_json::Error),
    PromptFileMissing(String),
    GitConfigRead(String, io::Error), // For reading .git/config or similar
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileRead(file, e) => write!(f, "Failed to read file '{}': {}", file, e),
            ConfigError::JsonParse(file, e) => write!(f, "Failed to parse JSON from file '{}': {}", file, e),
            ConfigError::PromptFileMissing(file) => write!(f, "Critical prompt file '{}' is missing.", file),
            ConfigError::GitConfigRead(context, e) => write!(f, "Failed to read Git configuration for {}: {}", context, e),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::FileRead(_, e) => Some(e),
            ConfigError::JsonParse(_, e) => Some(e),
            ConfigError::PromptFileMissing(_) => None,
            ConfigError::GitConfigRead(_, e) => Some(e),
        }
    }
}

// Git Command Errors
#[derive(Debug)]
pub enum GitError {
    CommandFailed(String, Option<i32>, String, String), // command, status_code, stdout, stderr
    DiffError(io::Error),
    NotARepository,
    NoStagedChanges,
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::CommandFailed(cmd, code, stdout, stderr) => {
                write!(f, "Git command '{}' failed", cmd)?;
                if let Some(c) = code {
                    write!(f, " with exit code {}", c)?;
                }
                if !stdout.is_empty() {
                    write!(f, "\nStdout:\n{}", stdout)?;
                }
                if !stderr.is_empty() {
                    write!(f, "\nStderr:\n{}", stderr)?;
                }
                Ok(())
            }
            GitError::DiffError(e) => write!(f, "Failed to get git diff: {}", e),
            GitError::NotARepository => write!(f, "Not a git repository (or any of the parent directories)."),
            GitError::NoStagedChanges => write!(f, "No changes staged for commit."),
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GitError::DiffError(e) => Some(e),
            _ => None,
        }
    }
}

// AI Interaction Errors
#[derive(Debug)]
pub enum AIError {
    RequestFailed(reqwest::Error),
    ResponseParseFailed(reqwest::Error), // Error during response.json()
    ApiResponseError(reqwest::StatusCode, String), // HTTP status was not success, String is response body
    NoChoiceInResponse,
    EmptyMessage,
}

impl std::fmt::Display for AIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AIError::RequestFailed(e) => write!(f, "AI API request failed: {}", e),
            AIError::ResponseParseFailed(e) => write!(f, "Failed to parse AI API JSON response: {}", e),
            AIError::ApiResponseError(status, body) => write!(f, "AI API responded with error {}: {}", status, body),
            AIError::NoChoiceInResponse => write!(f, "AI API response contained no choices."),
            AIError::EmptyMessage => write!(f, "AI returned an empty message."),
        }
    }
}

impl std::error::Error for AIError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AIError::RequestFailed(e) => Some(e),
            AIError::ResponseParseFailed(e) => Some(e),
            AIError::ApiResponseError(_, _) => None,
            _ => None,
        }
    }
}

// Converters from specific errors to AppError
impl From<ConfigError> for AppError {
    fn from(err: ConfigError) -> AppError {
        AppError::Config(err)
    }
}

impl From<GitError> for AppError {
    fn from(err: GitError) -> AppError {
        AppError::Git(err)
    }
}

impl From<AIError> for AppError {
    fn from(err: AIError) -> AppError {
        AppError::AI(err)
    }
}

// For convenience, if a function can return io::Error directly
impl From<io::Error> for AppError {
    fn from(err: io::Error) -> AppError {
        AppError::Io("unknown context".to_string(), err) // Encourage more specific context
    }
}
// Add From<reqwest::Error> and From<serde_json::Error> if needed directly in AppError context
// Or handle them within specific error types like AIError or ConfigError.

// Helper for converting Command output to GitError
pub fn map_command_error(
    cmd_str: &str,
    output: std::process::Output,
    status: std::process::ExitStatus,
) -> GitError {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    GitError::CommandFailed(cmd_str.to_string(), status.code(), stdout, stderr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    // Helper to create a dummy reqwest::Error
    // reqwest::Error is non-exhaustive. We generate one by causing a proxy parsing error.
    fn mock_reqwest_error() -> reqwest::Error {
        // Intentionally use a URL that will cause a parsing error when creating the proxy.
        // reqwest::Proxy::all itself returns Result<Proxy, reqwest::Error>
        let invalid_proxy_url = "::not_a_valid_url::"; // This is not a valid URL format.
        match reqwest::Proxy::all(invalid_proxy_url) {
            Ok(_) => panic!("Proxy::all should have failed for the invalid URL: {}", invalid_proxy_url),
            Err(e) => e, // This 'e' is the reqwest::Error we want
        }
    }

    // Helper to create a dummy serde_json::Error
    // serde_json::Error is also somewhat complex. We'll simulate a parsing error.
    fn mock_serde_json_error() -> serde_json::Error {
        serde_json::from_str::<serde_json::Value>("{invalid_json").err().unwrap()
    }

    #[test]
    fn test_config_error_display() {
        let file_name = "test_config.json".to_string();
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let json_err = mock_serde_json_error();

        let err_file_read = ConfigError::FileRead(file_name.clone(), io_err);
        assert_eq!(
            format!("{}", err_file_read),
            "Failed to read file 'test_config.json': file not found"
        );

        let err_json_parse = ConfigError::JsonParse(file_name.clone(), json_err);
        // The exact format of serde_json::Error can vary, so we check for key parts.
        assert!(format!("{}", err_json_parse)
            .starts_with("Failed to parse JSON from file 'test_config.json': "));

        let err_prompt_missing = ConfigError::PromptFileMissing("prompts/my_prompt".to_string());
        assert_eq!(
            format!("{}", err_prompt_missing),
            "Critical prompt file 'prompts/my_prompt' is missing."
        );
        
        let git_config_io_err = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");
        let err_git_config_read = ConfigError::GitConfigRead("user name".to_string(), git_config_io_err);
        assert_eq!(
            format!("{}", err_git_config_read),
            "Failed to read Git configuration for user name: permission denied"
        );
    }

    #[test]
    fn test_git_error_display() {
        let io_err = io::Error::new(io::ErrorKind::Other, "diff generation failed");
        let err_diff = GitError::DiffError(io_err);
        assert_eq!(
            format!("{}", err_diff),
            "Failed to get git diff: diff generation failed"
        );

        let err_not_repo = GitError::NotARepository;
        assert_eq!(
            format!("{}", err_not_repo),
            "Not a git repository (or any of the parent directories)."
        );

        let err_no_staged = GitError::NoStagedChanges;
        assert_eq!(format!("{}", err_no_staged), "No changes staged for commit.");

        let err_cmd_failed_simple = GitError::CommandFailed("git version".to_string(), Some(128), "".to_string(), "fatal error".to_string());
        let actual_str_simple = format!("{}", err_cmd_failed_simple);
        let expected_str_simple = "Git command 'git version' failed with exit code 128\nStderr:\nfatal error";

        // Debugging prints for err_cmd_failed_simple
        println!("Actual simple bytes: {:?}", actual_str_simple.as_bytes());
        println!("Expected simple bytes: {:?}", expected_str_simple.as_bytes());
        
        assert_eq!(actual_str_simple, expected_str_simple);
        
        let err_cmd_failed_full = GitError::CommandFailed("git status".to_string(), Some(0), "on branch master".to_string(), "warning".to_string());
        let actual_str_full = format!("{}", err_cmd_failed_full);
        let expected_str_full = "Git command 'git status' failed with exit code 0\nStdout:\non branch master\nStderr:\nwarning";

        // Debugging prints for err_cmd_failed_full
        println!("Actual full bytes: {:?}", actual_str_full.as_bytes());
        println!("Expected full bytes: {:?}", expected_str_full.as_bytes());

        assert_eq!(actual_str_full, expected_str_full);
        
        let err_cmd_failed_no_code = GitError::CommandFailed("git pull".to_string(), None, "".to_string(), "terminated".to_string());
        let actual_str_no_code = format!("{}", err_cmd_failed_no_code);
        let expected_str_no_code = "Git command 'git pull' failed\nStderr:\nterminated";

        // Debugging prints for err_cmd_failed_no_code
        println!("Actual no_code bytes: {:?}", actual_str_no_code.as_bytes());
        println!("Expected no_code bytes: {:?}", expected_str_no_code.as_bytes());
        
        assert_eq!(actual_str_no_code, expected_str_no_code);
    }

    #[test]
    fn test_ai_error_display() {
        let req_err = mock_reqwest_error();
        let err_request_failed = AIError::RequestFailed(req_err); // req_err is moved here
        assert!(format!("{}", err_request_failed).starts_with("AI API request failed: "));

        let parse_err = mock_reqwest_error(); // Simulate error during .json()
        let err_response_parse_failed = AIError::ResponseParseFailed(parse_err);
        assert!(format!("{}", err_response_parse_failed).starts_with("Failed to parse AI API JSON response: "));

        let err_api_response = AIError::ApiResponseError(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "Server meltdown".to_string());
        assert_eq!(
            format!("{}", err_api_response),
            "AI API responded with error 500 Internal Server Error: Server meltdown"
        );

        let err_no_choice = AIError::NoChoiceInResponse;
        assert_eq!(format!("{}", err_no_choice), "AI API response contained no choices.");
        
        let err_empty_message = AIError::EmptyMessage;
        assert_eq!(format!("{}", err_empty_message), "AI returned an empty message.");
    }

    #[test]
    fn test_app_error_display() {
        let config_err = ConfigError::PromptFileMissing("prompts/sys".to_string());
        let app_config_err = AppError::Config(config_err);
        assert_eq!(
            format!("{}", app_config_err),
            "Configuration error: Critical prompt file 'prompts/sys' is missing."
        );

        let git_err = GitError::NotARepository;
        let app_git_err = AppError::Git(git_err);
        assert_eq!(
            format!("{}", app_git_err),
            "Git command error: Not a git repository (or any of the parent directories)."
        );

        let ai_err = AIError::EmptyMessage;
        let app_ai_err = AppError::AI(ai_err);
        assert_eq!(
            format!("{}", app_ai_err),
            "AI interaction error: AI returned an empty message."
        );
        
        let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "pipe broke");
        let app_io_err = AppError::Io("writing to pipe".to_string(), io_err);
        assert_eq!(
            format!("{}", app_io_err),
            "I/O error while writing to pipe: pipe broke"
        );
    }
}