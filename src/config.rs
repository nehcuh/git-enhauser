use serde::Deserialize;
use std::{fs, io};
use std::path::Path;
use tracing::{error, info, warn};

use crate::errors::ConfigError;

const CONFIG_FILE_NAME: &str = "config.json";
const CONFIG_EXAMPLE_FILE_NAME: &str = "config.example.json";
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
        let config_example_path = Path::new(CONFIG_EXAMPLE_FILE_NAME);
        let prompt_path = Path::new(PROMPT_FILE_NAME);

        // Load defaults from config.example.json first
        let mut merged_config: PartialAppConfig = if config_example_path.exists() {
            info!("Loading default configuration from {}", CONFIG_EXAMPLE_FILE_NAME);
            let config_content = fs::read_to_string(config_example_path)
                .map_err(|e| ConfigError::FileRead(CONFIG_EXAMPLE_FILE_NAME.to_string(), e))?;
            serde_json::from_str(&config_content)
                .map_err(|e| ConfigError::JsonParse(CONFIG_EXAMPLE_FILE_NAME.to_string(), e))?
        } else {
             error!(
                "Default configuration file {} not found. Cannot load configuration.",
                CONFIG_EXAMPLE_FILE_NAME
            );
            // If the example file is missing, we can't even get defaults. This is an error.
            return Err(ConfigError::FileRead(CONFIG_EXAMPLE_FILE_NAME.to_string(), io::Error::new(io::ErrorKind::NotFound, "config.example.json not found")));
        };


        // If config.json exists, load it and merge (user settings override defaults)
        if config_path.exists() {
            info!("Loading user configuration from {}", CONFIG_FILE_NAME);
            let user_config_content = fs::read_to_string(config_path)
                .map_err(|e| ConfigError::FileRead(CONFIG_FILE_NAME.to_string(), e))?;
            let user_partial_config: PartialAppConfig = serde_json::from_str(&user_config_content)
                .map_err(|e| ConfigError::JsonParse(CONFIG_FILE_NAME.to_string(), e))?;

            // Merge user config into default config
            if user_partial_config.api_url.is_some() {
                merged_config.api_url = user_partial_config.api_url;
            }
            if user_partial_config.model_name.is_some() {
                merged_config.model_name = user_partial_config.model_name;
            }
            if user_partial_config.temperature.is_some() {
                merged_config.temperature = user_partial_config.temperature;
            }
            // api_key can be explicitly set to null or a value
            merged_config.api_key = user_partial_config.api_key.or(merged_config.api_key.take());

        } else {
            warn!(
                "Configuration file {} not found. Using default values from {}. Please create one based on {} for custom settings.",
                CONFIG_FILE_NAME, CONFIG_EXAMPLE_FILE_NAME, CONFIG_EXAMPLE_FILE_NAME
            );
        }

        // Handle the placeholder api_key from example config
        if let Some(api_key) = &merged_config.api_key {
            if api_key == "YOUR_API_KEY_IF_NEEDED" {
                merged_config.api_key = None;
                info!("Using default configuration, API key placeholder found in config.example.json. Treating as no API key.");
            }
        }


        let system_prompt = if prompt_path.exists() {
            info!("Loading system prompt from {}", PROMPT_FILE_NAME);
            fs::read_to_string(prompt_path)
                .map_err(|e| ConfigError::FileRead(PROMPT_FILE_NAME.to_string(), e))?
        } else {
            error!("System prompt file {} not found! AI generation might not work as expected.", PROMPT_FILE_NAME);
            return Err(ConfigError::PromptFileMissing(PROMPT_FILE_NAME.to_string()));
        };

        // Ensure required fields from merged_config are present before unwrapping
        let api_url = merged_config.api_url.ok_or_else(|| ConfigError::FieldMissing("api_url".to_string()))?;
        let model_name = merged_config.model_name.ok_or_else(|| ConfigError::FieldMissing("model_name".to_string()))?;
        let temperature = merged_config.temperature.ok_or_else(|| ConfigError::FieldMissing("temperature".to_string()))?;


        Ok(AppConfig {
            api_url,
            model_name,
            temperature,
            api_key: merged_config.api_key,
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
        create_example_config: bool, // New parameter
    ) -> PathBuf {
        let base_path = PathBuf::from(format!("target/test_temp_data/{}", test_name));
        if base_path.exists() {
            fs::remove_dir_all(&base_path).expect("Failed to remove test directory during setup");
        }

        if create_prompts_dir {
            fs::create_dir_all(base_path.join("prompts")).expect("Failed to create prompts directory during setup");
        } else {
            // Ensure base_path itself exists for config.json/config.example.json if prompts dir isn't needed
            fs::create_dir_all(&base_path).expect("Failed to create base directory during setup");
        }

        // Add this block to create config.example.json
        if create_example_config {
            let example_config_path = base_path.join(CONFIG_EXAMPLE_FILE_NAME);
            // Hardcode the example config content here for tests
            let example_content = r#"{
              "api_url": "http://localhost:11434/v1/chat/completions",
              "model_name": "qwen3:32b-q8_0",
              "temperature": 0.7,
              "api_key": "YOUR_API_KEY_IF_NEEDED"
            }"#;
            let mut file = File::create(example_config_path).expect("Failed to create config.example.json during setup");
            file.write_all(example_content.as_bytes()).expect("Failed to write to config.example.json during setup");
        }


        if let Some(content) = config_content {
            let mut file = File::create(base_path.join(CONFIG_FILE_NAME)).expect("Failed to create config.json during setup");
            file.write_all(content.as_bytes()).expect("Failed to write to config.json during setup");
        }

        if let Some(content) = prompt_content {
            // PROMPT_FILE_NAME includes "prompts/" prefix
            let prompt_path = base_path.join(PROMPT_FILE_NAME);
            // Ensure the prompts directory exists before creating the prompt file
            fs::create_dir_all(prompt_path.parent().expect("Failed to get prompt file parent directory")).expect("Failed to create prompts directory during setup");
            let mut file = File::create(prompt_path).expect("Failed to create prompt file during setup");
            file.write_all(content.as_bytes()).expect("Failed to write to prompt file during setup");
        }
        base_path
    }

    fn cleanup_test_environment(base_path: PathBuf) {
        if base_path.exists() {
            fs::remove_dir_all(&base_path).expect("Failed to clean up test directory");
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
        // Doesn't need example config as config.json is full
        let base_path = setup_test_environment(test_name, Some(config_json), Some(prompt_text), true, true);
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
        // Needs example config for defaults
        let base_path = setup_test_environment(test_name, Some(config_json), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_url, "http://partial.host/api");
        assert_eq!(config.model_name, "partial-model");
        assert_eq!(config.temperature, 0.7); // Should use default from example
        assert_eq!(config.api_key, None); // Should be None (placeholder in example)
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
        // Needs example config for defaults
        let base_path = setup_test_environment(test_name, Some(config_json), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_url, "http://localhost:11434/v1/chat/completions"); // Default from example
        assert_eq!(config.model_name, "qwen3:32b-q8_0"); // Default from example
        assert_eq!(config.temperature, 0.7); // Default from example
        assert_eq!(config.api_key, None); // Should be None (placeholder in example)
        assert_eq!(config.system_prompt, prompt_text);

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }


    #[test]
    fn test_load_no_config_file() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_no_config_file";
        let prompt_text = "No config file prompt";
        // Pass None for config_content, but needs example config for defaults
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_url, "http://localhost:11434/v1/chat/completions"); // Default from example
        assert_eq!(config.model_name, "qwen3:32b-q8_0"); // Default from example
        assert_eq!(config.temperature, 0.7); // Default from example
        assert_eq!(config.api_key, None); // Should be None (placeholder in example)
        assert_eq!(config.system_prompt, prompt_text);

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }

     #[test]
    fn test_load_no_config_and_no_example_file() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_no_config_and_no_example_file";
        let prompt_text = "Prompt text";
        // Pass None for config_content and prompt_content, and don't create example config or prompts dir
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), false, false);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            // It should fail because config.example.json is missing
            ConfigError::FileRead(path, _) => {
                 assert_eq!(path, CONFIG_EXAMPLE_FILE_NAME);
            }
            e => panic!("Expected FileRead error for example config, got {:?}", e),
        }

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }


    #[test]
    fn test_load_missing_prompt_file() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_missing_prompt_file";
        let config_json = r#"{}"#;
        // Pass None for prompt_content, and don't create prompts dir for it. Needs example config for base config.
        let base_path = setup_test_environment(test_name, Some(config_json), None, false, true);
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
        // Setup environment without creating the "prompts" directory. Needs example config for base config.
        let base_path = setup_test_environment(test_name, Some(config_json), None, false, true);
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
         // Needs example config as fallback, although the error is in config.json
        let base_path = setup_test_environment(test_name, Some(invalid_config_json), Some(prompt_text), true, true);
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
    fn test_load_invalid_example_config_json() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_invalid_example_config_json";
         // Invalid type in example config
        let invalid_example_config_json = r#"{
            "api_url": "http://invalid.json",
            "model_name": "invalid-model",
            "temperature": "not_a_float"
        }"#;
        let prompt_text = "Prompt text";
        // Needs invalid example config, no config.json
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true, false);
        let example_config_path = base_path.join(CONFIG_EXAMPLE_FILE_NAME);
        let mut file = File::create(example_config_path).unwrap();
        file.write_all(invalid_example_config_json.as_bytes()).unwrap();


        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            ConfigError::JsonParse(path, _) => {
                assert_eq!(path, CONFIG_EXAMPLE_FILE_NAME);
            }
            e => panic!("Expected JsonParse error for example config, got {:?}", e),
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
        // Needs example config as fallback, though config.json has the key
        let base_path = setup_test_environment(test_name, Some(config_json), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_key, None); // Serde should deserialize JSON null to Option::None

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_api_key_placeholder_becomes_none() {
         let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_api_key_placeholder_becomes_none";
        let prompt_text = "Prompt text";
        // Use config.example.json which has the placeholder
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.api_key, None); // Placeholder should be treated as None

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }
}