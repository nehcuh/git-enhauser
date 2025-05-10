// git-enhancer/src/ai_explainer.rs
use crate::config::AppConfig;
use crate::errors::AIError;
use crate::ai_utils::{ChatMessage, OpenAIChatRequest, OpenAIChatCompletionResponse, clean_ai_output};

const EXPLAIN_OUTPUT_SYSTEM_PROMPT: &str = r#"You are a helpful assistant integrated into a Git command-line enhancer.
The user has executed a Git command and received the following output.
Please explain this output clearly and concisely.
If the output indicates an error or a common misunderstanding, clarify it.
Focus on what the output means and what the user might want to do next.
Do not include any conversational pleasantries or self-references like "As an AI...".
Just provide the explanation directly."#;

const EXPLAIN_COMMAND_SYSTEM_PROMPT: &str = r#"You are a helpful assistant integrated into a Git command-line enhancer.
The user wants to understand a specific Git command.
Please explain the Git command provided by the user clearly and concisely.
Describe its purpose, common options (if any are apparent or highly relevant), and typical use cases.
If the command seems incomplete or potentially problematic, you can briefly note that.
Do not include any conversational pleasantries or self-references like "As an AI...".
Just provide the explanation for the command directly.
The user's command will follow."#;

/// Helper function to execute the AI request and process the response.
async fn execute_ai_request(
    config: &AppConfig,
    messages: Vec<ChatMessage>,
) -> Result<String, AIError> {
    let request_payload = OpenAIChatRequest {
        model: config.model_name.clone(),
        messages,
        temperature: Some(config.temperature), // Using temperature from general AppConfig
        stream: false,
    };

    if let Ok(json_string) = serde_json::to_string_pretty(&request_payload) {
        tracing::debug!("Sending JSON payload to AI for explanation:\n{}", json_string);
    } else {
        tracing::warn!("Failed to serialize AI request payload for debugging.");
    }

    let client = reqwest::Client::new();
    let mut request_builder = client.post(&config.api_url);

    // Add Authorization header if api_key is present
    if let Some(api_key) = &config.api_key {
        if !api_key.is_empty() {
            tracing::debug!("Using API key for AI explanation request.");
            request_builder = request_builder.bearer_auth(api_key);
        }
    }
    
    let openai_response = request_builder
        .json(&request_payload)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AI explainer request failed during send: {}", e);
            // This error could be a network issue, DNS resolution failure, etc.
            // AIError::RequestFailed is a general error for reqwest issues.
            // AIError::ExplainerNetworkError could be used if a more specific categorization is needed
            // and can be reliably determined from `e`.
            AIError::RequestFailed(e) 
        })?;

    if !openai_response.status().is_success() {
        let status_code = openai_response.status();
        let body = openai_response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error body from AI response".to_string());
        tracing::error!(
            "AI explainer API request failed with status {}: {}",
            status_code,
            body
        );
        return Err(AIError::ApiResponseError(status_code, body));
    }

    // Successfully received a response, now parse it.
    match openai_response.json::<OpenAIChatCompletionResponse>().await {
        Ok(response_data) => {
            if let Some(choice) = response_data.choices.get(0) {
                let original_content = &choice.message.content;
                if original_content.trim().is_empty() {
                    tracing::warn!("AI explainer returned an empty message content.");
                    Err(AIError::EmptyMessage)
                } else {
                    let cleaned_content = clean_ai_output(original_content);
                    tracing::debug!("Cleaned AI explanation received: \"{}\"", cleaned_content.chars().take(100).collect::<String>()); // Log snippet
                    Ok(cleaned_content)
                }
            } else {
                tracing::warn!("No choices found in AI explainer response.");
                Err(AIError::NoChoiceInResponse)
            }
        }
        Err(e) => {
            tracing::error!("Failed to parse JSON response from AI explainer: {}", e);
            // This error occurs if the response body is not valid JSON matching OpenAIChatCompletionResponse
            Err(AIError::ResponseParseFailed(e))
        }
    }
}

/// Takes the raw output from a Git command (typically its help text)
/// and returns an AI-generated explanation for that output.
pub async fn explain_git_command_output(
    config: &AppConfig,
    command_output: &str,
) -> Result<String, AIError> {
    if command_output.trim().is_empty() {
        // This is not an error, but a valid case where there's nothing to explain.
        return Ok(
            "The command produced no output for the AI to explain. \
            It might be a command that doesn't print to stdout/stderr on success, \
            or it requires specific conditions to produce output."
                .to_string(),
        );
    }

    tracing::debug!("Requesting AI explanation for command output (first 200 chars):\n---\n{}\n---", command_output.chars().take(200).collect::<String>());

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: EXPLAIN_OUTPUT_SYSTEM_PROMPT.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: command_output.to_string(), // Send the full output
        },
    ];
    
    execute_ai_request(config, messages).await
}

/// Takes a Git command (as a sequence of its parts/arguments)
/// and returns an AI-generated explanation of what that command does.
pub async fn explain_git_command(
    config: &AppConfig,
    command_parts: &[String],
) -> Result<String, AIError> {
    if command_parts.is_empty() {
        // This is not an error from AI's perspective but an invalid input to this function.
        return Ok("No command parts provided for the AI to explain.".to_string());
    }

    let command_to_explain = format!("git {}", command_parts.join(" "));
    tracing::debug!("Requesting AI explanation for command: {}", command_to_explain);
    
    let user_message_content = command_to_explain;

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: EXPLAIN_COMMAND_SYSTEM_PROMPT.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_message_content,
        },
    ];

    execute_ai_request(config, messages).await
}