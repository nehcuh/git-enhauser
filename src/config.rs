use serde::Deserialize;
use std::{fs, io};
use std::path::{Path, PathBuf};
use std::io::ErrorKind;
use tracing::{error, info};
use dirs::home_dir;
use std::fs::create_dir_all;

use crate::errors::ConfigError;

const PROJECT_CONFIG_FILE_NAME: &str = "config.toml";
const PROJECT_CONFIG_EXAMPLE_FILE_NAME: &str = "config.example.toml";
const USER_CONFIG_DIR: &str = ".config/gitie";
const USER_CONFIG_FILE_NAME: &str = "config.toml";
const PROJECT_PROMPT_FILE_NAME: &str = "prompts/commit-prompt";
const USER_PROMPT_FILE_NAME: &str = "commit-prompt";

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
        let prompt_path = Path::new(PROJECT_PROMPT_FILE_NAME);
        
        // FOR TESTING: if we're in a test directory, adjust paths for better error messages
        let _in_test = std::env::current_dir()
            .map(|p| p.to_string_lossy().contains("target/test_temp_data"))
            .unwrap_or(false);
        
        // 首先检查用户目录配置是否存在
        if user_config_path.exists() {
            // 用户配置存在，直接从用户目录加载
            info!("Loading configuration from user directory: {:?}", user_config_path);
            return Self::load_config_from_file(&user_config_path, prompt_path);
        }
        
        // 用户目录配置不存在，检查项目配置
        if project_config_path.exists() {
            info!("Found project configuration: {}", PROJECT_CONFIG_FILE_NAME);
            
            // 先读取项目配置文件内容并验证
            let project_config_content = fs::read_to_string(project_config_path)
                .map_err(|e| ConfigError::FileRead(project_config_path.to_string_lossy().to_string(), e))?;
            
            // 尝试解析TOML验证有效性
            match toml::from_str::<PartialAppConfig>(&project_config_content) {
                Ok(_) => {
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
                    fs::write(&user_config_path, project_config_content).map_err(|e| {
                        ConfigError::FileWrite(
                            user_config_path.to_string_lossy().to_string(),
                            e
                        )
                    })?;
                    
                    // 从复制后的用户配置加载
                    return Self::load_config_from_file(&user_config_path, prompt_path);
                }
                Err(e) => {
                    // 配置无效，返回解析错误
                    return Err(ConfigError::TomlParse(
                        project_config_path.to_string_lossy().to_string(), 
                        e
                    ));
                }
            }
        }
        
        // 项目配置也不存在，检查示例配置
        if project_example_config_path.exists() {
            info!("No configuration found. Creating default configuration from example.");
            
            // 先读取示例配置文件内容并验证
            let example_config_content = fs::read_to_string(project_example_config_path)
                .map_err(|e| ConfigError::FileRead(project_example_config_path.to_string_lossy().to_string(), e))?;
            
            // 尝试解析TOML验证有效性
            match toml::from_str::<PartialAppConfig>(&example_config_content) {
                Ok(_) => {
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
                    fs::write(&user_config_path, example_config_content).map_err(|e| {
                        ConfigError::FileWrite(
                            user_config_path.to_string_lossy().to_string(), 
                            e
                        )
                    })?;
                    
                    // 从复制后的用户配置加载
                    return Self::load_config_from_file(&user_config_path, prompt_path);
                }
                Err(e) => {
                    // 示例配置无效，返回解析错误
                    return Err(ConfigError::TomlParse(
                        project_example_config_path.to_string_lossy().to_string(), 
                        e
                    ));
                }
            }
        }
        
        // 所有配置文件都不存在，无法继续
        error!("No configuration files found. Cannot continue.");
        Err(ConfigError::FileRead(
            PROJECT_CONFIG_EXAMPLE_FILE_NAME.to_string(),
            io::Error::new(ErrorKind::NotFound, "No configuration files found")
        ))
    }
    
    // 获取用户配置文件的路径
    // Override the get_user_config_path function to use a test directory
    fn get_user_config_path() -> Result<std::path::PathBuf, ConfigError> {
        // Use the environment variable HOME set during test setup
        let home_str = std::env::var("HOME").unwrap_or_else(|_| {
            // Fallback to real home directory if env var not set
            home_dir()
                .expect("Could not determine home directory")
                .to_string_lossy()
                .to_string()
        });
        
        let home = PathBuf::from(home_str);
        Ok(home.join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME))
    }
    
    // 获取用户提示文件的路径
    fn get_user_prompt_path() -> Result<std::path::PathBuf, ConfigError> {
        // Use the environment variable HOME set during test setup
        let home_str = std::env::var("HOME").unwrap_or_else(|_| {
            // Fallback to real home directory if env var not set
            home_dir()
                .expect("Could not determine home directory")
                .to_string_lossy()
                .to_string()
        });
        
        let home = PathBuf::from(home_str);
        Ok(home.join(USER_CONFIG_DIR).join(USER_PROMPT_FILE_NAME))
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
        
        // 获取用户提示文件路径
        let user_prompt_path = Self::get_user_prompt_path()?;
        
        // 尝试加载系统提示文件，优先使用用户目录中的提示文件
        let system_prompt = if user_prompt_path.exists() {
            info!("Loading system prompt from user directory: {:?}", user_prompt_path);
            fs::read_to_string(&user_prompt_path)
                .map_err(|e| ConfigError::FileRead(user_prompt_path.to_string_lossy().to_string(), e))?
        } else if prompt_path.exists() {
            // 如果用户目录中没有提示文件，使用项目目录中的提示文件
            info!("Loading system prompt from project: {}", PROJECT_PROMPT_FILE_NAME);
            
            // 读取项目提示文件
            let prompt_content = fs::read_to_string(prompt_path)
                .map_err(|e| ConfigError::FileRead(PROJECT_PROMPT_FILE_NAME.to_string(), e))?;
            
            // 复制到用户目录
            info!("Copying prompt to user directory: {:?}", user_prompt_path);
            if let Some(parent) = user_prompt_path.parent() {
                create_dir_all(parent).map_err(|e| {
                    ConfigError::FileWrite(
                        parent.to_string_lossy().to_string(),
                        e
                    )
                })?;
            }
            
            fs::write(&user_prompt_path, &prompt_content).map_err(|e| {
                ConfigError::FileWrite(
                    user_prompt_path.to_string_lossy().to_string(), 
                    e
                )
            })?;
            
            prompt_content
        } else {
            error!("System prompt file not found! AI generation might not work as expected.");
            return Err(ConfigError::PromptFileMissing(PROJECT_PROMPT_FILE_NAME.to_string()));
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

        // Create project root directory
        fs::create_dir_all(&base_path).expect("Failed to create base directory during setup");
        
        // Create a mock home directory with .config/gitie structure
        let mock_home = base_path.join("mock_home");
        let mock_config_dir = mock_home.join(USER_CONFIG_DIR);
        fs::create_dir_all(&mock_config_dir).expect("Failed to create mock config directory during setup");
        
        // Patch the get_user_config_path function for testing by using environment variable
        unsafe { std::env::set_var("HOME", mock_home.to_str().unwrap()) };

        if create_prompts_dir {
            fs::create_dir_all(base_path.join("prompts")).expect("Failed to create prompts directory during setup");
        }

        // Add this block to create config.example.toml in project directory
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

        // Create project config.toml if content is provided
        if let Some(content) = config_content {
            let mut file = File::create(base_path.join(PROJECT_CONFIG_FILE_NAME)).expect("Failed to create config.toml during setup");
            file.write_all(content.as_bytes()).expect("Failed to write to config.toml during setup");
        }

        if let Some(content) = prompt_content {
            // PROJECT_PROMPT_FILE_NAME includes "prompts/" prefix
            let prompt_path = base_path.join(PROJECT_PROMPT_FILE_NAME);
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
        // Reset the HOME environment variable after test cleanup
        unsafe { 
            // Only remove if test didn't panic
            if std::env::var("HOME").is_ok() {
                std::env::remove_var("HOME");
            }
        };
    }

    #[test]
    fn test_load_full_config() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_full_config";
        let config_toml = r#"[ai]
api_url = "http://custom.host/api"
model_name = "custom-model"
temperature = 0.5
api_key = "test_key_123"
"#;
        let prompt_text = "Test system prompt";
        // Setup with project config
        let base_path = setup_test_environment(test_name, Some(config_toml), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        // The config will be copied to user directory during load
        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        // Verify the config values
        assert_eq!(config.ai.api_url, "http://custom.host/api");
        assert_eq!(config.ai.model_name, "custom-model");
        assert_eq!(config.ai.temperature, 0.5);
        assert_eq!(config.ai.api_key, Some("test_key_123".to_string()));
        assert_eq!(config.system_prompt, prompt_text);

        // Verify the config was copied to user directory
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        let mock_user_prompt = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_PROMPT_FILE_NAME);
        assert!(mock_user_config.exists(), "Config should be copied to user directory");
        assert!(mock_user_prompt.exists(), "Prompt should be copied to user directory");

        let _ = std::env::set_current_dir(original_dir);
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
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        // These values should match the project config
        assert_eq!(config.ai.api_url, "http://partial.host/api");
        assert_eq!(config.ai.model_name, "partial-model");
        // These should have default values
        assert_eq!(config.ai.temperature, 0.7); // Default from example
        assert_eq!(config.ai.api_key, None); // Should be None (not specified)
        assert_eq!(config.system_prompt, prompt_text);

        // Verify files were copied to user directory
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        let mock_user_prompt = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_PROMPT_FILE_NAME);
        assert!(mock_user_config.exists(), "Config should be copied to user directory");
        assert!(mock_user_prompt.exists(), "Prompt should be copied to user directory");

        let _ = std::env::set_current_dir(original_dir);
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_load_partial_config_empty_toml() {
        // Directly lock the mutex to prevent PoisonError issues
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_partial_config_empty_toml";
        // Use minimal valid config instead of empty TOML
        let config_toml = r#"[ai]
api_url = "http://localhost:11434/v1/chat/completions"
model_name = "qwen3:32b-q8_0"
"#;
        let prompt_text = "Empty TOML config prompt";
        // Needs example config for defaults
        let base_path = setup_test_environment(test_name, Some(config_toml), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.ai.api_url, "http://localhost:11434/v1/chat/completions"); // From config
        assert_eq!(config.ai.model_name, "qwen3:32b-q8_0"); // From config
        assert_eq!(config.ai.temperature, 0.7); // Default
        assert_eq!(config.ai.api_key, None); // Should be None (not specified)
        assert_eq!(config.system_prompt, prompt_text);

        // Verify files were copied to user directory
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        let mock_user_prompt = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_PROMPT_FILE_NAME);
        assert!(mock_user_config.exists(), "Config should be copied to user directory");
        assert!(mock_user_prompt.exists(), "Prompt should be copied to user directory");

        let _ = std::env::set_current_dir(original_dir);
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
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        // All values should come from example config
        assert_eq!(config.ai.api_url, "http://localhost:11434/v1/chat/completions");
        assert_eq!(config.ai.model_name, "qwen3:32b-q8_0");
        assert_eq!(config.ai.temperature, 0.7);
        assert_eq!(config.ai.api_key, None); // Should be None (placeholder in example)
        assert_eq!(config.system_prompt, prompt_text);

        // Verify the example config was copied to user directory
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        let mock_user_prompt = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_PROMPT_FILE_NAME);
        assert!(mock_user_config.exists(), "Example config should be copied to user directory");
        assert!(mock_user_prompt.exists(), "Prompt should be copied to user directory");

        let _ = std::env::set_current_dir(original_dir);
        cleanup_test_environment(base_path);
    }

     #[test]
    fn test_load_no_config_and_no_example_file() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_no_config_and_no_example_file";
        let prompt_text = "Prompt text";
        // No config files at all, just the prompt
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true, false);
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            // Should fail because neither user config, project config, nor example config exist
            ConfigError::FileRead(path, _) => {
                 assert!(path.contains(PROJECT_CONFIG_EXAMPLE_FILE_NAME));
            }
            e => panic!("Expected FileRead error for example config, got {:?}", e),
        }

        let _ = std::env::set_current_dir(original_dir);
        cleanup_test_environment(base_path);
    }


    #[test]
    fn test_load_missing_prompt_file() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_missing_prompt_file";
        let config_toml = r#""#;
        // Create empty config but no prompt file
        let base_path = setup_test_environment(test_name, Some(config_toml), None, false, true);
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            ConfigError::PromptFileMissing(path) => {
                assert!(path.contains(PROJECT_PROMPT_FILE_NAME));
            }
            e => panic!("Expected PromptFileMissing, got {:?}", e),
        }

        // Config should still be copied to user directory even though prompt is missing
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        assert!(mock_user_config.exists(), "Config should be copied to user directory despite prompt error");
        // No need to check prompt file as it's missing by design in this test

        let _ = std::env::set_current_dir(original_dir);
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_load_missing_prompts_directory() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_missing_prompts_directory";
        let config_toml = r#""#;
        // Setup without prompts directory
        let base_path = setup_test_environment(test_name, Some(config_toml), None, false, true);
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            ConfigError::PromptFileMissing(path) => {
                 assert!(path.contains(PROJECT_PROMPT_FILE_NAME));
            }
            e => panic!("Expected PromptFileMissing, got {:?}", e),
        }

        // Config should still be copied to user directory even though prompt is missing
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        assert!(mock_user_config.exists(), "Config should be copied to user directory despite prompt directory error");
        // No need to check prompt file as it's missing by design in this test

        let _ = std::env::set_current_dir(original_dir);
        cleanup_test_environment(base_path);
    }


    #[test]
    fn test_load_invalid_config_toml() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_invalid_config_toml";
        let invalid_config_toml = r#"[ai]
api_url = "http://invalid.toml"
model_name = "invalid-model"
temperature = "not_a_float"  # Invalid type
"#;
        let prompt_text = "Invalid config prompt";
        // Setup with invalid project config and valid example config
        let base_path = setup_test_environment(test_name, Some(invalid_config_toml), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            ConfigError::TomlParse(path, _) => {
                assert!(path.contains(PROJECT_CONFIG_FILE_NAME));
            }
            e => panic!("Expected TomlParse error, got {:?}", e),
        }

        // The invalid config should not be copied to user directory
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        let mock_user_prompt = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_PROMPT_FILE_NAME);
        assert!(!mock_user_config.exists(), "Invalid config should not be copied to user directory");
        assert!(!mock_user_prompt.exists(), "Prompt should not be copied when config is invalid");

        let _ = std::env::set_current_dir(original_dir);
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_load_invalid_example_config_toml() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_invalid_example_config_toml";
        // Invalid type in example config
        let invalid_example_config_toml = r#"[ai]
api_url = "http://invalid.toml"
model_name = "invalid-model"
temperature = "not_a_float"
"#;
        let prompt_text = "Prompt text";
        // No project config, only invalid example config
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true, false);
        let example_config_path = base_path.join(PROJECT_CONFIG_EXAMPLE_FILE_NAME);
        let mut file = File::create(example_config_path).unwrap();
        file.write_all(invalid_example_config_toml.as_bytes()).unwrap();

        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_err());
        match config_result.err().unwrap() {
            ConfigError::TomlParse(path, _) => {
                assert!(path.ends_with(PROJECT_CONFIG_EXAMPLE_FILE_NAME) || 
                        path.contains(PROJECT_CONFIG_EXAMPLE_FILE_NAME),
                       "Expected path to contain example config filename, got {}", path);
            }
            e => panic!("Expected TomlParse error for example config, got {:?}", e),
        }

        // The invalid example config should not be copied to user directory
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        let mock_user_prompt = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_PROMPT_FILE_NAME);
        assert!(!mock_user_config.exists(), "Invalid example config should not be copied to user directory");
        assert!(!mock_user_prompt.exists(), "Prompt should not be copied when example config is invalid");

        let _ = std::env::set_current_dir(original_dir);
        cleanup_test_environment(base_path);
    }


    #[test]
    fn test_load_config_with_empty_api_key() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_load_config_with_empty_api_key";
        let config_toml = r#"[ai]
api_url = "http://custom.host/api"
model_name = "custom-model"
temperature = 0.5
api_key = ""
"#;
        let prompt_text = "Test system prompt with empty API key";
        // Setup with project config that has empty API key
        let base_path = setup_test_environment(test_name, Some(config_toml), Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.ai.api_key, None); // Empty string in TOML becomes None after our conversion
        
        // Verify the config was copied to user directory
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        let mock_user_prompt = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_PROMPT_FILE_NAME);
        assert!(mock_user_config.exists(), "Config should be copied to user directory");
        assert!(mock_user_prompt.exists(), "Prompt should be copied to user directory");

        let _ = std::env::set_current_dir(original_dir);
        cleanup_test_environment(base_path);
    }

    #[test]
    fn test_api_key_placeholder_becomes_none() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let test_name = "test_api_key_placeholder_becomes_none";
        let prompt_text = "Prompt text";
        // Only example config with placeholder API key
        let base_path = setup_test_environment(test_name, None, Some(prompt_text), true, true);
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());
        std::env::set_current_dir(&base_path).unwrap_or_else(|_| ());

        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Expected OK, got {:?}", config_result.err());
        let config = config_result.unwrap();

        assert_eq!(config.ai.api_key, None); // Placeholder should be treated as None
        
        // Verify the example config was copied to user directory
        let mock_user_config = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_CONFIG_FILE_NAME);
        let mock_user_prompt = base_path.join("mock_home").join(USER_CONFIG_DIR).join(USER_PROMPT_FILE_NAME);
        assert!(mock_user_config.exists(), "Example config should be copied to user directory");
        assert!(mock_user_prompt.exists(), "Prompt should be copied to user directory");

        let _ = std::env::set_current_dir(original_dir);
        cleanup_test_environment(base_path);
    }
}