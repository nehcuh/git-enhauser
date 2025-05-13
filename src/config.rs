use serde::Deserialize;
use std::{fs, io};
use std::path::Path;
use std::io::ErrorKind;
use tracing::{error, info};
use dirs::home_dir;
use std::fs::create_dir_all;

use crate::errors::ConfigError;

const PROJECT_CONFIG_FILE_NAME: &str = "config.toml";
const PROJECT_CONFIG_EXAMPLE_FILE_NAME: &str = "config.example.toml";
const USER_CONFIG_DIR: &str = ".config/gitie";
const USER_CONFIG_FILE_NAME: &str = "config.toml";
const PROMPT_FILE_NAME: &str = "prompts/commit-prompt";

// AI服务的配置
#[derive(Deserialize, Debug, Clone, Default)]
pub struct AIConfig {
    pub api_url: String,
    pub model_name: String,
    pub temperature: f32,
    pub api_key: Option<String>, // Made Option in case it's not always needed or provided
}

// 应用的总体配置
#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub ai: AIConfig,
    
    #[serde(skip)] // System prompt is loaded separately
    pub system_prompt: String,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        // 1. 尝试从用户目录加载配置
        let user_config_path = Self::get_user_config_path()?;
        let project_config_path = Path::new(PROJECT_CONFIG_FILE_NAME);
        let project_example_config_path = Path::new(PROJECT_CONFIG_EXAMPLE_FILE_NAME);
        let prompt_path = Path::new(PROMPT_FILE_NAME);
        
        // 首先检查用户目录配置是否存在
        if user_config_path.exists() {
            // 用户配置存在，直接从用户目录加载
            info!("Loading configuration from user directory: {:?}", user_config_path);
            return Self::load_config_from_file(&user_config_path, prompt_path);
        }
        
        // 用户目录配置不存在，检查项目配置
        if project_config_path.exists() {
            info!("Found project configuration: {}", PROJECT_CONFIG_FILE_NAME);
            info!("Copying to user directory: {:?}", user_config_path);
            
            // 确保用户配置目录存在
            if let Some(parent) = user_config_path.parent() {
                create_dir_all(parent).map_err(|e| {
                    ConfigError::FileWrite(
                        parent.to_string_lossy().to_string(),
                        e
                    )
                })?;
            }
            
            // 复制项目配置到用户目录
            fs::copy(project_config_path, &user_config_path).map_err(|e| {
                ConfigError::FileWrite(
                    user_config_path.to_string_lossy().to_string(),
                    e
                )
            })?;
            
            // 从复制后的用户配置加载
            return Self::load_config_from_file(&user_config_path, prompt_path);
        }
        
        // 项目配置也不存在，检查示例配置
        if project_example_config_path.exists() {
            info!("No configuration found. Creating default configuration from example.");
            info!("Copying {} to {:?}", PROJECT_CONFIG_EXAMPLE_FILE_NAME, user_config_path);
            
            // 确保用户配置目录存在
            if let Some(parent) = user_config_path.parent() {
                create_dir_all(parent).map_err(|e| {
                    ConfigError::FileWrite(
                        parent.to_string_lossy().to_string(),
                        e
                    )
                })?;
            }
            
            // 复制示例配置到用户目录
            fs::copy(project_example_config_path, &user_config_path).map_err(|e| {
                ConfigError::FileWrite(
                    user_config_path.to_string_lossy().to_string(), 
                    e
                )
            })?;
            
            // 从复制后的用户配置加载
            return Self::load_config_from_file(&user_config_path, prompt_path);
        }
        
        // 所有配置文件都不存在，无法继续
        error!("No configuration files found. Cannot continue.");
        Err(ConfigError::FileRead(
            PROJECT_CONFIG_EXAMPLE_FILE_NAME.to_string(),
            io::Error::new(ErrorKind::NotFound, "No configuration files found")
        ))
    }
    
    // 获取用户配置文件的路径
    fn get_user_config_path() -> Result<std::path::PathBuf, ConfigError> {
        let home = home_dir().ok_or_else(|| {
            ConfigError::FileRead(
                "~".to_string(),
                io::Error::new(ErrorKind::NotFound, "Home directory not found")
            )
        })?;
        
        Ok(home.join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME))
    }
    
    // 从指定文件加载配置
    fn load_config_from_file(config_path: &Path, prompt_path: &Path) -> Result<Self, ConfigError> {
        // 读取配置文件
        let config_content = fs::read_to_string(config_path)
            .map_err(|e| ConfigError::FileRead(config_path.to_string_lossy().to_string(), e))?;
        
        // 解析TOML
        let mut partial_config: PartialAppConfig = toml::from_str(&config_content)
            .map_err(|e| ConfigError::TomlParse(config_path.to_string_lossy().to_string(), e))?;
        
        // 处理API密钥占位符
        if let Some(ai) = &mut partial_config.ai {
            if let Some(api_key) = &ai.api_key {
                if api_key == "YOUR_API_KEY_IF_NEEDED" || api_key.is_empty() {
                    ai.api_key = None;
                    info!("API key placeholder or empty string found. Treating as no API key.");
                }
            }
        }
        
        // 确保ai部分存在
        if partial_config.ai.is_none() {
            partial_config.ai = Some(PartialAIConfig::default());
        }
        
        // 加载系统提示文件
        let system_prompt = if prompt_path.exists() {
            info!("Loading system prompt from {}", PROMPT_FILE_NAME);
            fs::read_to_string(prompt_path)
                .map_err(|e| ConfigError::FileRead(PROMPT_FILE_NAME.to_string(), e))?
        } else {
            error!("System prompt file {} not found! AI generation might not work as expected.", PROMPT_FILE_NAME);
            return Err(ConfigError::PromptFileMissing(PROMPT_FILE_NAME.to_string()));
        };
        
        // 验证并处理AI配置
        let partial_ai_config = partial_config.ai.unwrap_or_default();
        
        // 获取必填字段值或使用默认值
        let api_url = partial_ai_config.api_url.unwrap_or_default();
        let model_name = partial_ai_config.model_name.unwrap_or_default();
        let temperature = partial_ai_config.temperature.unwrap_or(0.7);
        
        // 检查必填字段
        if api_url.is_empty() {
            return Err(ConfigError::FieldMissing("ai.api_url".to_string()));
        }
        if model_name.is_empty() {
            return Err(ConfigError::FieldMissing("ai.model_name".to_string()));
        }
        
        // 构建最终配置
        let ai_config = AIConfig {
            api_url,
            model_name,
            temperature,
            api_key: partial_ai_config.api_key,
        };
        
        Ok(AppConfig {
            ai: ai_config,
            system_prompt,
        })
    }
}

// AI配置的部分加载辅助结构体
#[derive(Deserialize, Debug, Default, Clone)]
struct PartialAIConfig {
    #[serde(default)]
    api_url: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    api_key: Option<String>,
}

// 部分加载的配置辅助结构体
#[derive(Deserialize, Debug, Default)]
struct PartialAppConfig {
    ai: Option<PartialAIConfig>,
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
            // Ensure base_path itself exists for config.toml/config.example.toml if prompts dir isn't needed
            fs::create_dir_all(&base_path).expect("Failed to create base directory during setup");
        }

        // Add this block to create config.example.toml
        if create_example_config {
            let example_config_path = base_path.join(PROJECT_CONFIG_EXAMPLE_FILE_NAME);
            // Hardcode the example config content here for tests
            let example_content = r#"[ai]
api_url = "http://localhost:11434/v1/chat/completions"
model_name = "qwen3:32b-q8_0"
temperature = 0.7
api_key = "YOUR_API_KEY_IF_NEEDED"
"#;
            let mut file = File::create(example_config_path).expect("Failed to create config.example.toml during setup");
            file.write_all(example_content.as_bytes()).expect("Failed to write to config.example.toml during setup");
        }


        if let Some(content) = config_content {
            let mut file = File::create(base_path.join(PROJECT_CONFIG_FILE_NAME)).expect("Failed to create config.toml during setup");
            file.write_all(content.as_bytes()).expect("Failed to write to config.toml during setup");
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
        let config_toml = r#"[ai]
api_url = "http://custom.host/api"
model_name = "custom-model"
temperature = 0.5
api_key = "test_key_123"
"#;
        let prompt_text = "Test system prompt";
        // Doesn't need example config as config.toml is full
        let base_path = setup_test_environment(test_name, Some(config_toml), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.ai.api_url, "http://custom.host/api");
        assert_eq!(config.ai.model_name, "custom-model");
        assert_eq!(config.ai.temperature, 0.5);
        assert_eq!(config.ai.api_key, Some("test_key_123".to_string()));
        assert_eq!(config.system_prompt, prompt_text);

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_load_partial_config_missing_temp_and_key() {
        // Directly lock the mutex to prevent PoisonError issues
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_partial_config_missing_temp_and_key";
        let config_toml = r#"[ai]
api_url = "http://partial.host/api"
model_name = "partial-model"
"#; // Missing temperature and api_key
        let prompt_text = "Partial config prompt";
        // Needs example config for defaults
        let base_path = setup_test_environment(test_name, Some(config_toml), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.ai.api_url, "http://partial.host/api");
        assert_eq!(config.ai.model_name, "partial-model");
        assert_eq!(config.ai.temperature, 0.7); // Should use default from example
        assert_eq!(config.ai.api_key, None); // Should be None (placeholder in example)
        assert_eq!(config.system_prompt, prompt_text);

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_load_partial_config_empty_toml() {
        // Directly lock the mutex to prevent PoisonError issues
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_partial_config_empty_toml";
        let config_toml = r#""#; // Empty TOML
        let prompt_text = "Empty TOML config prompt";
        // Needs example config for defaults
        let base_path = setup_test_environment(test_name, Some(config_toml), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.ai.api_url, "http://localhost:11434/v1/chat/completions"); // Default from example
        assert_eq!(config.ai.model_name, "qwen3:32b-q8_0"); // Default from example
        assert_eq!(config.ai.temperature, 0.7); // Default from example
        assert_eq!(config.ai.api_key, None); // Should be None (placeholder in example)
        assert_eq!(config.system_prompt, prompt_text);

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }


    #[test]
    fn test_load_no_config_file() {
        // Directly lock the mutex to prevent PoisonError issues
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_no_config_file";
        let prompt_text = "No config file prompt";
        // Pass None for config_content, but needs example config for defaults
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.ai.api_url, "http://localhost:11434/v1/chat/completions"); // Default from example
        assert_eq!(config.ai.model_name, "qwen3:32b-q8_0"); // Default from example
        assert_eq!(config.ai.temperature, 0.7); // Default from example
        assert_eq!(config.ai.api_key, None); // Should be None (placeholder in example)
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
            // It should fail because config.example.toml is missing
            ConfigError::FileRead(path, _) => {
                 assert_eq!(path, PROJECT_CONFIG_EXAMPLE_FILE_NAME);
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
        let config_toml = r#""#;
        // Pass None for prompt_content, and don't create prompts dir for it. Needs example config for base config.
        let base_path = setup_test_environment(test_name, Some(config_toml), None, false, true);
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
        let config_toml = r#""#;
        // Setup environment without creating the "prompts" directory. Needs example config for base config.
        let base_path = setup_test_environment(test_name, Some(config_toml), None, false, true);
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
    fn test_load_invalid_config_toml() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_invalid_config_toml";
        let invalid_config_toml = r#"[ai]
api_url = "http://invalid.toml"
model_name = "invalid-model"
temperature = "not_a_float"  # Invalid type
"#;
        let prompt_text = "Invalid config prompt";
         // Needs example config as fallback, although the error is in config.toml
        let base_path = setup_test_environment(test_name, Some(invalid_config_toml), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            ConfigError::TomlParse(path, _) => {
                assert_eq!(path, PROJECT_CONFIG_FILE_NAME);
            }
            e => panic!("Expected TomlParse error, got {:?}", e),
        }

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_load_invalid_example_config_toml() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let test_name = "test_load_invalid_example_config_toml";
         // Invalid type in example config
        let invalid_example_config_toml = r#"[ai]
api_url = "http://invalid.toml"
model_name = "invalid-model"
temperature = "not_a_float"
"#;
        let prompt_text = "Prompt text";
        // Needs invalid example config, no config.toml
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true, false);
        let example_config_path = base_path.join(PROJECT_CONFIG_EXAMPLE_FILE_NAME);
        let mut file = File::create(example_config_path).unwrap();
        file.write_all(invalid_example_config_toml.as_bytes()).unwrap();


        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            ConfigError::TomlParse(path, _) => {
                assert_eq!(path, PROJECT_CONFIG_EXAMPLE_FILE_NAME);
            }
            e => panic!("Expected TomlParse error for example config, got {:?}", e),
        }

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }


    #[test]
    fn test_load_config_with_empty_api_key() {
        // Directly lock the mutex to prevent PoisonError issues
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_config_with_empty_api_key";
        let config_toml = r#"[ai]
api_url = "http://custom.host/api"
model_name = "custom-model"
temperature = 0.5
api_key = ""
"#;
        let prompt_text = "Test system prompt with empty API key";
        // Needs example config as fallback, though config.toml has the key
        let base_path = setup_test_environment(test_name, Some(config_toml), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.ai.api_key, None); // Empty string in TOML becomes None after our conversion

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_api_key_placeholder_becomes_none() {
        // Directly lock the mutex to prevent PoisonError issues
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_api_key_placeholder_becomes_none";
        let prompt_text = "Prompt text";
        // Use config.example.toml which has the placeholder
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base_path).unwrap();

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.ai.api_key, None); // Placeholder should be treated as None

        std::env::set_current_dir(original_dir).unwrap();
        cleanup_test_environment(base_path);
    }
}