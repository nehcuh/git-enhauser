use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Mutex;

// Helper to get the path to the compiled binary
fn get_binary_path() -> PathBuf {
    let cargo_manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir_name = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    cargo_manifest_dir
        .join("target")
        .join(target_dir_name)
        .join(env!("CARGO_PKG_NAME"))
}

// Struct to manage a temporary test directory with a .git folder
struct TestRepo {
    path: PathBuf, // Should store the absolute, canonicalized path
    original_dir: PathBuf,
}

impl TestRepo {
    fn new(test_name: &str) -> Self {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let base_temp_path = project_root
            .join("target")
            .join("test_integration_temp_data");

        // Ensure base_temp_path itself exists
        if !base_temp_path.exists() {
            fs::create_dir_all(&base_temp_path).expect(&format!(
                "Failed to create base temp dir: {:?}",
                base_temp_path
            ));
        }

        let repo_path_relative = base_temp_path.join(test_name);

        if repo_path_relative.exists() {
            fs::remove_dir_all(&repo_path_relative).expect(&format!(
                "Failed to remove old test repo: {:?}",
                repo_path_relative
            ));
        }

        fs::create_dir_all(&repo_path_relative).expect(&format!(
            "Failed to create test repo dir: {:?}",
            repo_path_relative
        ));

        // Check if it *really* exists and is a directory
        if !repo_path_relative.exists() || !repo_path_relative.is_dir() {
            panic!(
                "Test repo path was not created or is not a directory: {:?}",
                repo_path_relative
            );
        }

        // Canonicalize the path to make it absolute and resolve symlinks, etc.
        let repo_path_absolute = fs::canonicalize(&repo_path_relative).expect(&format!(
            "Failed to canonicalize repo path: {:?}",
            repo_path_relative
        ));

        // Initialize a new git repository here
        let init_output = Command::new("git")
            .arg("init")
            .current_dir(&repo_path_absolute) // Run in the new repo's directory
            .output()
            .expect("Failed to execute git init");
        if !init_output.status.success() {
            panic!(
                "git init failed: {:?}\\nStdout: {}\\nStderr: {}",
                init_output.status,
                String::from_utf8_lossy(&init_output.stdout),
                String::from_utf8_lossy(&init_output.stderr)
            );
        }

        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&repo_path_absolute).expect("Failed to set current dir to test repo");

        // Create dummy config.json and commit-prompt
        let config_content = r#"{
            "api_url": "http://localhost:12345/v1/mock",
            "model_name": "mock-model",
            "temperature": 0.1,
            "api_key": null
        }"#;
        let prompt_content = "This is a mock system prompt.";

        // Now use repo_path_absolute for file operations if CWD wasn't changed yet,
        // or relative paths if CWD is already repo_path_absolute.
        // Since CWD is now repo_path_absolute, relative paths are fine here.
        fs::write(PathBuf::from("config.json"), config_content)
            .expect("Failed to write mock config.json");
        fs::create_dir_all(PathBuf::from("prompts")).expect("Failed to create prompts dir");
        fs::write(PathBuf::from("prompts/commit-prompt"), prompt_content)
            .expect("Failed to write mock commit-prompt");

        TestRepo {
            path: repo_path_absolute, // Store the absolute path
            original_dir,
        }
    }

    fn run_git_enhancer(&self, args: &[&str]) -> Output {
        let binary_path = get_binary_path();
        println!("Attempting to run binary: {:?}", binary_path); // Debug print
        if !binary_path.exists() {
            panic!(
                "git-enhancer binary not found at: {:?}. Please ensure the project is built (e.g., with `cargo build` or `cargo test` which builds dependencies).",
                binary_path
            );
        }
        Command::new(binary_path)
            .args(args)
            .current_dir(&self.path) // Ensure command runs in the test repo context
            .env("RUST_LOG", "info") // Explicitly set log level for the subprocess
            .output()
            .expect("Failed to execute git-enhancer")
    }

    #[allow(dead_code)] // May be used by other tests
    fn git_command(&self, args: &[&str]) -> Output {
        Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .expect(&format!("Failed to execute git command: {:?}", args))
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        env::set_current_dir(&self.original_dir).expect("Failed to restore original dir");
        // fs::remove_dir_all(&self.path).expect("Failed to clean up test repo"); // Cleanup can be noisy, enable if needed
    }
}

// Mutex for tests that might interact with global state or shared resources,
// though individual TestRepo instances should provide good isolation.
static INTEGRATION_TEST_MUTEX: Mutex<()> = Mutex::new(());
