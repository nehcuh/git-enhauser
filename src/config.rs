use serde::Deserialize;
use std::fs;
use std::path::Path;
use tracing::{error, info, warn};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ConfigError; // Import ConfigError for matching
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Mutex;

    // Mutex to ensure tests changing current directory run serially
    // This is crucial because AppConfig::load() relies on relative paths from the current directory.
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn setup_test_environment(
        test_name: &str,
        config_content: Option<&str>,
        prompt_content: Option<&str>,
        create_prompts_dir: bool,
    ) -> PathBuf {
        let base_path = PathBuf::from(format!("target/test_temp_data/{}", test_name));
        if base_path.exists() {
            fs::remove_dir_all(&base_path).unwrap();
        }

        if create_prompts_dir {
            fs::create_dir_all(base_path.join("prompts")).unwrap();
        } else {
            // Ensure base_path itself exists for config.json if prompts dir isn't needed
            fs::create_dir_all(&base_path).unwrap();
        }


        if let Some(content) = config_content {
            let mut file = File::create(base_path.join(CONFIG_FILE_NAME)).unwrap();
            file.write_all(content.as_bytes()).unwrap();
        }

        if let Some(content) = prompt_content {
            // PROMPT_FILE_NAME includes "prompts/" prefix
            let prompt_path = base_path.join(PROMPT_FILE_NAME);
            let mut file = File::create(prompt_path).unwrap();
            file.write_all(content.as_bytes()).unwrap();
        }
        base_path
    }

    fn cleanup_test_environment(base_path: PathBuf) {
        if base_path.exists() {
            fs::remove_dir_all(&base_path).unwrap();
        }
    }

    #[test]
    fn test_load_full_config() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_full_config";
        let config_json = r#"{
            "api_url": "http://custom.host/api",
            "model_name": "custom-model",
            "temperature": 0.5,
            "api_key": "test_key_123"
        }"#;
        let prompt_text = "Test system prompt";
        let base_path = setup_test_environment(test_name, Some(config_json), Some(prompt_text), true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_url, "http://custom.host/api");
        assert_eq!(config.model_name, "custom-model");
        assert_eq!(config.temperature, 0.5);
        assert_eq!(config.api_key, Some("test_key_123".to_string()));
        assert_eq!(config.system_prompt, prompt_text);

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_load_partial_config_missing_temp_and_key() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_partial_config_missing_temp_and_key";
        let config_json = r#"{
            "api_url": "http://partial.host/api",
            "model_name": "partial-model"
        }"#; // Missing temperature and api_key
        let prompt_text = "Partial config prompt";
        let base_path = setup_test_environment(test_name, Some(config_json), Some(prompt_text), true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_url, "http://partial.host/api");
        assert_eq!(config.model_name, "partial-model");
        assert_eq!(config.temperature, DEFAULT_TEMPERATURE); // Should use default
        assert_eq!(config.api_key, None); // Should be None
        assert_eq!(config.system_prompt, prompt_text);

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }
    
    #[test]
    fn test_load_partial_config_empty_json() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_partial_config_empty_json";
        let config_json = r#"{}"#; // Empty JSON
        let prompt_text = "Empty JSON config prompt";
        let base_path = setup_test_environment(test_name, Some(config_json), Some(prompt_text), true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_url, DEFAULT_API_URL);
        assert_eq!(config.model_name, DEFAULT_MODEL_NAME);
        assert_eq!(config.temperature, DEFAULT_TEMPERATURE);
        assert_eq!(config.api_key, None);
        assert_eq!(config.system_prompt, prompt_text);

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }


    #[test]
    fn test_load_no_config_file() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_no_config_file";
        let prompt_text = "No config file prompt";
        // Pass None for config_content
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_url, DEFAULT_API_URL);
        assert_eq!(config.model_name, DEFAULT_MODEL_NAME);
        assert_eq!(config.temperature, DEFAULT_TEMPERATURE);
        assert_eq!(config.api_key, None);
        assert_eq!(config.system_prompt, prompt_text);

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_load_missing_prompt_file() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_missing_prompt_file";
        let config_json = r#"{}"#;
        // Pass None for prompt_content, and don't create prompts dir for it
        let base_path = setup_test_environment(test_name, Some(config_json), None, false); 
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            ConfigError::PromptFileMissing(path) => {
                assert_eq!(path, PROMPT_FILE_NAME);
            }
            e => panic!("Expected PromptFileMissing, got {:?}", e),
        }

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }
    
    #[test]
    fn test_load_missing_prompts_directory() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_missing_prompts_directory";
        let config_json = r#"{}"#;
        // Setup environment without creating the "prompts" directory
        let base_path = setup_test_environment(test_name, Some(config_json), None, false);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            // Depending on fs behavior, this might be FileRead for the prompt file itself
            // or PromptFileMissing if the check is simply path.exists()
            // The current code `prompt_path.exists()` will lead to PromptFileMissing
            ConfigError::PromptFileMissing(path) => {
                 assert_eq!(path, PROMPT_FILE_NAME);
            }
            e => panic!("Expected PromptFileMissing, got {:?}", e),
        }

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }


    #[test]
    fn test_load_invalid_config_json() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_invalid_config_json";
        let invalid_config_json = r#"{
            "api_url": "http://invalid.json",
            "model_name": "invalid-model",
            "temperature": "not_a_float"  // Invalid type
        }"#;
        let prompt_text = "Invalid config prompt";
        let base_path = setup_test_environment(test_name, Some(invalid_config_json), Some(prompt_text), true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            ConfigError::JsonParse(path, _) => {
                assert_eq!(path, CONFIG_FILE_NAME);
            }
            e => panic!("Expected JsonParse error, got {:?}", e),
        }

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }
    
    #[test]
    fn test_load_config_with_null_api_key() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_config_with_null_api_key";
        let config_json = r#"{
            "api_url": "http://custom.host/api",
            "model_name": "custom-model",
            "temperature": 0.5,
            "api_key": null
        }"#;
        let prompt_text = "Test system prompt with null API key";
        let base_path = setup_test_environment(test_name, Some(config_json), Some(prompt_text), true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_key, None); // Serde should deserialize JSON null to Option::None

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }
}

use crate::errors::ConfigError;

const DEFAULT_API_URL: &str = "http://localhost:11434/v1/chat/completions";
const DEFAULT_MODEL_NAME: &str = "qwen3:32b-q8_0";
const DEFAULT_TEMPERATURE: f32 = 0.7;
const CONFIG_FILE_NAME: &str = "config.json";
const PROMPT_FILE_NAME: &str = "prompts/commit-prompt";

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub api_url: String,
    pub model_name: String,
    pub temperature: f32,
    pub api_key: Option<String>, // Made Option in case it's not always needed or provided
    #[serde(skip)] // System prompt is loaded separately
    pub system_prompt: String,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Path::new(CONFIG_FILE_NAME);
        let prompt_path = Path::new(PROMPT_FILE_NAME);

        let mut partial_config: PartialAppConfig = if config_path.exists() {
            info!("Loading configuration from {}", CONFIG_FILE_NAME);
            let config_content = fs::read_to_string(config_path)
                .map_err(|e| ConfigError::FileRead(CONFIG_FILE_NAME.to_string(), e))?;
            serde_json::from_str(&config_content)
                .map_err(|e| ConfigError::JsonParse(CONFIG_FILE_NAME.to_string(), e))?
        } else {
            warn!(
                "Configuration file {} not found. Using default values. Please create one based on config.example.json for custom settings.",
                CONFIG_FILE_NAME
            );
            PartialAppConfig::default()
        };

        // Fill in defaults if values are missing from the config file
        if partial_config.api_url.is_none() {
            partial_config.api_url = Some(DEFAULT_API_URL.to_string());
        }
        if partial_config.model_name.is_none() {
            partial_config.model_name = Some(DEFAULT_MODEL_NAME.to_string());
        }
        if partial_config.temperature.is_none() {
            partial_config.temperature = Some(DEFAULT_TEMPERATURE);
        }
        // api_key remains None if not provided, which is fine.

        let system_prompt = if prompt_path.exists() {
            info!("Loading system prompt from {}", PROMPT_FILE_NAME);
            fs::read_to_string(prompt_path)
                .map_err(|e| ConfigError::FileRead(PROMPT_FILE_NAME.to_string(), e))?
        } else {
            error!("System prompt file {} not found! AI generation might not work as expected.", PROMPT_FILE_NAME);
            // You might want to return an error here or use a very basic default prompt.
            // For now, returning an error as it's critical.
            return Err(ConfigError::PromptFileMissing(PROMPT_FILE_NAME.to_string()));
        };

        Ok(AppConfig {
            api_url: partial_config.api_url.unwrap(), // Should be set by now
            model_name: partial_config.model_name.unwrap(), // Should be set by now
            temperature: partial_config.temperature.unwrap(), // Should be set by now
            api_key: partial_config.api_key,
            system_prompt,
        })
    }
}

// Helper struct to allow for optional fields during deserialization from config.json
#[derive(Deserialize, Debug, Default)]
struct PartialAppConfig {
    api_url: Option<String>,
    model_name: Option<String>,
    temperature: Option<f32>,
    api_key: Option<String>,
}