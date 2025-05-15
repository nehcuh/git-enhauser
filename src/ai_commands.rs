use crate::ai_utils::{OpenAIChatCompletionResponse, OpenAIChatRequest, ChatMessage};
use crate::errors::{AppError, AIError};
use crate::types::CommandOutput;
use crate::config::AppConfig;
use reqwest;
use tracing;

/// Processes a git command with AI to generate explanations or enhancements
///
/// # Arguments
///
/// * `command` - The git command string
/// * `config` - The application configuration
///
/// # Returns
///
/// * `Result<String, AppError>` - The AI-generated explanation or enhancement
pub async fn process_git_command_with_ai(command: &str, config: &AppConfig) -> Result<String, AppError> {
    tracing::info!("Processing git command with AI: {}", command);
    
    // Create the AI request
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: config.system_prompt.clone(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!("Explain the following git command: {}", command),
        },
    ];
    
    let request = OpenAIChatRequest {
        model: config.model_name.clone(),
        messages,
        temperature: config.temperature,
        max_tokens: config.max_tokens.unwrap_or(1000),
    };
    
    // Send the request to AI service
    let response = send_ai_request(&request, config).await?;
    
    // Extract the response content
    extract_ai_response_content(response)
}

/// Processes git command output with AI to provide context and explanations
///
/// # Arguments
///
/// * `command` - The git command that was executed
/// * `output` - The output from the git command
/// * `config` - The application configuration
///
/// # Returns
///
/// * `Result<String, AppError>` - The AI-generated explanation
pub async fn process_git_output_with_ai(
    command: &str, 
    output: &CommandOutput, 
    config: &AppConfig
) -> Result<String, AppError> {
    tracing::info!("Processing git output with AI. Command: {}", command);
    
    // Create the AI request
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: config.system_prompt.clone(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "Explain the following git command output:\n\nCommand: {}\n\nOutput:\n{}\n\nError (if any):\n{}", 
                command, 
                output.stdout, 
                output.stderr
            ),
        },
    ];
    
    let request = OpenAIChatRequest {
        model: config.model_name.clone(),
        messages,
        temperature: config.temperature,
        max_tokens: config.max_tokens.unwrap_or(1500),
    };
    
    // Send the request to AI service
    let response = send_ai_request(&request, config).await?;
    
    // Extract the response content
    extract_ai_response_content(response)
}

/// Generates a commit message suggestion based on git diff
///
/// # Arguments
///
/// * `diff` - The git diff content
/// * `config` - The application configuration
///
/// # Returns
///
/// * `Result<String, AppError>` - The AI-generated commit message suggestion
pub async fn generate_commit_message(diff: &str, config: &AppConfig) -> Result<String, AppError> {
    tracing::info!("Generating commit message with AI");
    
    let truncated_diff = if diff.len() > 8000 {
        tracing::warn!("Diff is too large ({} chars), truncating to 8000 chars", diff.len());
        format!("{}... (truncated, too large)", &diff[0..7997])
    } else {
        diff.to_string()
    };
    
    // Create the AI request
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are a helpful assistant that generates concise, informative git commit messages based on code changes. Follow conventional commit format.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!("Generate a commit message for these changes:\n\n{}", truncated_diff),
        },
    ];
    
    let request = OpenAIChatRequest {
        model: config.model_name.clone(),
        messages,
        temperature: 0.5, // Lower temperature for more focused output
        max_tokens: 200,  // Commit messages should be concise
    };
    
    // Send the request to AI service
    let response = send_ai_request(&request, config).await?;
    
    // Extract the response content
    extract_ai_response_content(response)
}

/// Sends a request to the AI service
///
/// # Arguments
///
/// * `request` - The OpenAI chat request
/// * `config` - The application configuration
///
/// # Returns
///
/// * `Result<OpenAIChatCompletionResponse, AppError>` - The AI response or an error
async fn send_ai_request(
    request: &OpenAIChatRequest, 
    config: &AppConfig
) -> Result<OpenAIChatCompletionResponse, AppError> {
    let client = reqwest::Client::new();
    
    let api_key = config.api_key.as_ref()
        .ok_or_else(|| AppError::AI(AIError::ExplainerConfigurationError(
            "API key is required but not set. Please set it in your config.".to_string()
        )))?;
    
    tracing::debug!("Sending request to AI API at {}", config.api_url);
    
    let response = client.post(&config.api_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(request)
        .send()
        .await
        .map_err(|e| AppError::AI(AIError::ExplainerNetworkError(
            format!("Failed to connect to AI service: {}", e)
        )))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        
        return Err(AppError::AI(AIError::ExplainerNetworkError(
            format!("AI service returned error ({}): {}", status, error_text)
        )));
    }
    
    response.json::<OpenAIChatCompletionResponse>()
        .await
        .map_err(|e| AppError::AI(AIError::ExplainerNetworkError(
            format!("Failed to parse AI service response: {}", e)
        )))
}

/// Extracts the content from an AI response
///
/// # Arguments
///
/// * `response` - The OpenAI chat completion response
///
/// # Returns
///
/// * `Result<String, AppError>` - The extracted content or an error
fn extract_ai_response_content(response: OpenAIChatCompletionResponse) -> Result<String, AppError> {
    if response.choices.is_empty() {
        return Err(AppError::AI(AIError::ExplanationGenerationFailed(
            "AI returned an empty response".to_string()
        )));
    }
    
    Ok(response.choices[0].message.content.clone())
}

/// Extracts code blocks from AI response
///
/// # Arguments
///
/// * `content` - The AI response content
///
/// # Returns
///
/// * `Vec<String>` - Vector of extracted code blocks
pub fn extract_code_blocks(content: &str) -> Vec<String> {
    let mut code_blocks = Vec::new();
    let mut in_code_block = false;
    let mut current_block = String::new();
    
    for line in content.lines() {
        if line.trim().starts_with("```") {
            if in_code_block {
                // End of code block
                in_code_block = false;
                if !current_block.is_empty() {
                    code_blocks.push(current_block.trim().to_string());
                    current_block = String::new();
                }
            } else {
                // Start of code block
                in_code_block = true;
            }
        } else if in_code_block {
            current_block.push_str(line);
            current_block.push('\n');
        }
    }
    
    code_blocks
}