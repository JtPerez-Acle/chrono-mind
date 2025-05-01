#[cfg(test)]
mod cli_tests {

    use std::process::Command;
    use std::fs;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn test_cli_help() {
        let output = Command::new("cargo")
            .args(["run", "--", "--help"])
            .output()
            .expect("Failed to execute command");

        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");

        assert!(stdout.contains("A temporal vector storage solution for AI applications"));
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains("Options:"));
        assert!(output.status.success());
    }

    #[test]
    fn test_cli_version() {
        let output = Command::new("cargo")
            .args(["run", "--", "--version"])
            .output()
            .expect("Failed to execute command");

        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");

        assert!(stdout.contains("ChronoMind Vector Store"));
        assert!(output.status.success());
    }

    #[test]
    fn test_cli_save_and_load() {
        // Create a temporary directory for test files
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("test_vectors.json");
        let file_path_str = file_path.to_str().expect("Invalid path");

        // Create a test vector file
        let test_vector_json = r#"{
            "id": "test_vector_1",
            "data": [0.1, 0.2, 0.3, 0.4],
            "importance": 0.8,
            "context": "test_context",
            "timestamp": "2023-01-01T00:00:00Z"
        }"#;

        let input_file = temp_dir.path().join("input.json");
        let mut file = fs::File::create(&input_file).expect("Failed to create input file");
        file.write_all(test_vector_json.as_bytes()).expect("Failed to write to input file");
        let input_file_str = input_file.to_str().expect("Invalid path");

        // Save the vector
        let save_output = Command::new("cargo")
            .args(["run", "--", "save", "--input", input_file_str, "--output", file_path_str, "--dimensions", "4", "--normalize"])
            .output()
            .expect("Failed to execute save command");

        assert!(save_output.status.success(), "Save command failed: {}", String::from_utf8_lossy(&save_output.stderr));
        assert!(file_path.exists(), "Output file was not created");

        // Query the vector
        let query_output = Command::new("cargo")
            .args(["run", "--", "query", "--file", file_path_str, "--vector", "[0.1, 0.2, 0.3, 0.4]", "--limit", "1", "--normalize"])
            .output()
            .expect("Failed to execute query command");

        let query_stdout = String::from_utf8(query_output.stdout).expect("Invalid UTF-8");

        assert!(query_output.status.success(), "Query command failed: {}", String::from_utf8_lossy(&query_output.stderr));
        assert!(query_stdout.contains("test_vector_1"), "Query did not return the expected vector");

        // Clean up
        temp_dir.close().expect("Failed to clean up temp directory");
    }

    #[test]
    fn test_cli_stats() {
        // Create a temporary directory for test files
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("test_vectors.json");
        let file_path_str = file_path.to_str().expect("Invalid path");

        // Create a test vector file
        let test_vector_json = r#"{
            "id": "test_vector_1",
            "data": [0.1, 0.2, 0.3, 0.4],
            "importance": 0.8,
            "context": "test_context",
            "timestamp": "2023-01-01T00:00:00Z"
        }"#;

        let input_file = temp_dir.path().join("input.json");
        let mut file = fs::File::create(&input_file).expect("Failed to create input file");
        file.write_all(test_vector_json.as_bytes()).expect("Failed to write to input file");
        let input_file_str = input_file.to_str().expect("Invalid path");

        // Save the vector
        let save_output = Command::new("cargo")
            .args(["run", "--", "save", "--input", input_file_str, "--output", file_path_str, "--dimensions", "4", "--normalize"])
            .output()
            .expect("Failed to execute save command");

        assert!(save_output.status.success(), "Save command failed: {}", String::from_utf8_lossy(&save_output.stderr));

        // Get stats
        let stats_output = Command::new("cargo")
            .args(["run", "--", "stats", "--file", file_path_str])
            .output()
            .expect("Failed to execute stats command");

        let stats_stdout = String::from_utf8(stats_output.stdout).expect("Invalid UTF-8");

        assert!(stats_output.status.success(), "Stats command failed: {}", String::from_utf8_lossy(&stats_output.stderr));
        assert!(stats_stdout.contains("Total memories:"), "Stats did not include total memories");
        assert!(stats_stdout.contains("1"), "Stats did not show the correct number of memories");

        // Clean up
        temp_dir.close().expect("Failed to clean up temp directory");
    }
}
