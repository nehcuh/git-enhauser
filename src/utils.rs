use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing;

use crate::errors::AppError;

/// Reads the entire contents of a file into a string
///
/// # Arguments
///
/// * `path` - The path to the file to read
///
/// # Returns
///
/// * `Result<String, AppError>` - The file contents or an error
pub fn read_file_to_string(path: impl AsRef<Path>) -> Result<String, AppError> {
    let path = path.as_ref();
    let mut file = File::open(path).map_err(|e| {
        AppError::Io(
            format!("Failed to open file: {}", path.display()),
            e
        )
    })?;
    
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|e| {
        AppError::Io(
            format!("Failed to read file: {}", path.display()),
            e
        )
    })?;
    
    Ok(contents)
}

/// Writes a string to a file, creating the file if it doesn't exist
///
/// # Arguments
///
/// * `path` - The path to write to
/// * `contents` - The string to write
///
/// # Returns
///
/// * `Result<(), AppError>` - Success or an error
pub fn write_string_to_file(path: impl AsRef<Path>, contents: &str) -> Result<(), AppError> {
    let path = path.as_ref();
    
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Io(
                    format!("Failed to create directory: {}", parent.display()),
                    e
                )
            })?;
        }
    }
    
    let mut file = File::create(path).map_err(|e| {
        AppError::Io(
            format!("Failed to create file: {}", path.display()),
            e
        )
    })?;
    
    file.write_all(contents.as_bytes()).map_err(|e| {
        AppError::Io(
            format!("Failed to write to file: {}", path.display()),
            e
        )
    })?;
    
    Ok(())
}

/// Checks if a file exists
///
/// # Arguments
///
/// * `path` - The path to check
///
/// # Returns
///
/// * `bool` - True if the file exists
pub fn file_exists(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    path.exists() && path.is_file()
}

/// Gets the current timestamp in seconds since the Unix epoch
///
/// # Returns
///
/// * `Result<u64, AppError>` - The timestamp or an error
pub fn get_unix_timestamp() -> Result<u64, AppError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| AppError::Time(format!("Failed to get system time: {}", e)))
}

/// Formats a string for console output with optional color
///
/// # Arguments
///
/// * `text` - The text to format
/// * `is_error` - Whether this is an error message (default: false)
///
/// # Returns
///
/// * `String` - The formatted string
pub fn format_output(text: &str, is_error: bool) -> String {
    if is_error {
        format!("\x1b[31m{}\x1b[0m", text) // Red text for errors
    } else {
        text.to_string()
    }
}

/// Truncates a string to a maximum length, adding an ellipsis if truncated
///
/// # Arguments
///
/// * `s` - The string to truncate
/// * `max_length` - The maximum length
///
/// # Returns
///
/// * `String` - The truncated string
pub fn truncate_string(s: &str, max_length: usize) -> String {
    if s.len() <= max_length {
        s.to_string()
    } else {
        let mut truncated = s.chars().take(max_length - 3).collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

/// Finds the project root directory (where .git is located)
///
/// # Returns
///
/// * `Result<PathBuf, AppError>` - The project root path or an error
pub fn find_project_root() -> Result<PathBuf, AppError> {
    let mut current_dir = env::current_dir().map_err(|e| {
        AppError::Io("Failed to get current directory".to_string(), e)
    })?;
    
    // Keep going up until we find a .git directory
    loop {
        let git_dir = current_dir.join(".git");
        if git_dir.exists() && git_dir.is_dir() {
            return Ok(current_dir);
        }
        
        // Go up one directory
        if !current_dir.pop() {
            // We've reached the root of the filesystem without finding .git
            return Err(AppError::Generic(
                "Not in a git repository (or any parent directory)".to_string()
            ));
        }
    }
}

/// Safely creates a temporary file with the given content
///
/// # Arguments
///
/// * `prefix` - Prefix for the temp file name
/// * `content` - Content to write to the file
///
/// # Returns
///
/// * `Result<PathBuf, AppError>` - Path to the temporary file or an error
pub fn create_temp_file(prefix: &str, content: &str) -> Result<PathBuf, AppError> {
    let temp_dir = env::temp_dir();
    let timestamp = get_unix_timestamp()?;
    let random_suffix = rand::random::<u16>();
    
    let filename = format!("{}_{:x}_{:x}", prefix, timestamp, random_suffix);
    let temp_path = temp_dir.join(filename);
    
    write_string_to_file(&temp_path, content)?;
    
    tracing::debug!("Created temporary file at: {}", temp_path.display());
    Ok(temp_path)
}

/// Safely joins path components, handling errors
///
/// # Arguments
///
/// * `base` - The base path
/// * `components` - Path components to join
///
/// # Returns
///
/// * `PathBuf` - The joined path
pub fn safe_path_join(base: impl AsRef<Path>, components: &[impl AsRef<Path>]) -> PathBuf {
    let mut result = base.as_ref().to_path_buf();
    for component in components {
        result = result.join(component);
    }
    result
}