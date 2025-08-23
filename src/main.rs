use clap::{Parser, Subcommand};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use ubi::UbiBuilder;

// CLI structure using clap
#[derive(Parser)]
#[command(name = "bina")]
#[command(about = "Manages binary installations in XDG_BIN_HOME", long_about = None)]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Checks availability of binaries in XDG_BIN_HOME
    Check {
        /// Check the latest release version from GitHub
        #[arg(long)]
        latest: bool,
    },
    /// Downloads a specified binary using ubi
    Get {
        /// The name of the binary to download
        bin_name: String,
    },
    /// Downloads all missing binaries
    GetMissing,
}

// Structure to represent a binary entry in the TOML file
#[derive(Debug, Serialize, Deserialize)]
struct Binary {
    name: String,
    repo: String,
    exe: String,
    version_arg: String,
}

// Structure to represent the TOML file content
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    binaries: Vec<Binary>,
}

// Define the data mapping for binaries from TOML file
fn get_data() -> HashMap<String, [String; 3]> {
    let config_dir = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let home = env::var("HOME").expect("HOME environment variable not set");
        format!("{}/.config", home)
    });
    let toml_path = format!("{}/bina/binaries.toml", config_dir);

    let toml_str = fs::read_to_string(&toml_path)
        .expect(&format!("Failed to read binaries.toml from {}", toml_path));
    let config: Config = toml::from_str(&toml_str)
        .expect(&format!("Failed to parse binaries.toml from {}", toml_path));

    let mut data = HashMap::new();
    for binary in config.binaries {
        data.insert(
            binary.name.clone(),
            [binary.repo, binary.exe, binary.version_arg],
        );
    }
    data
}

// Ensures XDG_BIN_HOME is set and the directory exists
fn ensure_bin_directory() -> bool {
    let bin_directory = env::var("XDG_BIN_HOME").unwrap_or_default();
    if bin_directory.is_empty() {
        println!("Error: XDG_BIN_HOME environment variable is not set");
        return false;
    }

    let path = Path::new(&bin_directory);
    if !path.exists() {
        println!("Creating directory {}...", bin_directory);
        match fs::create_dir_all(path) {
            Ok(_) => true,
            Err(_) => {
                println!("Failed to create directory {}", bin_directory);
                return false;
            }
        }
    } else {
        true
    }
}

// Checks the latest release version for a GitHub repository asynchronously
async fn check_latest_release(repo: &str) -> String {
    let client = Client::new();
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    match client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "reqwest")
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<Value>().await {
                    Ok(json) => json["tag_name"].as_str().unwrap_or("Error").to_string(),
                    Err(_) => "Error".to_string(),
                }
            } else {
                "Error".to_string()
            }
        }
        Err(_) => "Error".to_string(),
    }
}

// Downloads a specified binary using ubi
async fn binary_get(
    bin_name: &str,
    data: &HashMap<String, [String; 3]>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !ensure_bin_directory() {
        return Err("Failed to ensure bin directory".into());
    }

    let bin_data = match data.get(bin_name) {
        Some(data) => data,
        None => {
            return Err(format!("Binary '{}' not found in data", bin_name).into());
        }
    };
    let xdg_bin_home = env::var("XDG_BIN_HOME").expect("XDG_BIN_HOME not set");

    let mut ubi = UbiBuilder::new()
        .project(&bin_data[0])
        .install_dir(&xdg_bin_home)
        .exe(&bin_data[1])
        .build()?;

    ubi.install_binary().await?;
    println!("Successfully downloaded {}", bin_name);
    Ok(())
}

// Checks availability of binaries in XDG_BIN_HOME
async fn binary_check(
    data: &HashMap<String, [String; 3]>,
    check_latest: bool,
) -> Vec<HashMap<String, String>> {
    if !ensure_bin_directory() {
        return vec![];
    }

    let xdg_bin_home = env::var("XDG_BIN_HOME").expect("XDG_BIN_HOME not set");
    let paths = fs::read_dir(&xdg_bin_home).expect("Failed to read XDG_BIN_HOME");
    let binaries: Vec<String> = paths
        .filter_map(|entry| {
            entry
                .ok()
                .map(|e| e.file_name().into_string().unwrap_or_default())
        })
        .collect();

    let re = Regex::new(r"(\d+\.\d+\.\d+)").expect("Invalid regex");

    let mut results = vec![];
    for (bin_name, bin_data) in data {
        let mut result = HashMap::new();
        result.insert("Binary".to_string(), bin_name.to_string());

        if binaries.contains(&bin_name.to_string()) {
            let output = Command::new(bin_name)
                .arg(&bin_data[2])
                .output()
                .expect("Failed to execute binary");
            let version_output = String::from_utf8_lossy(&output.stdout);
            let version = re
                .captures(&version_output)
                .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                .unwrap_or("-".to_string());
            result.insert("Status".to_string(), "Found".to_string());
            result.insert("Version".to_string(), version);
            if check_latest {
                let latest = check_latest_release(&bin_data[0]).await;
                let latest_version = re
                    .captures(&latest)
                    .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                    .unwrap_or("-".to_string());
                result.insert("Latest".to_string(), latest_version);
            }
        } else {
            result.insert("Status".to_string(), "Not Found".to_string());
            result.insert("Version".to_string(), "-".to_string());
            if check_latest {
                let latest = check_latest_release(&bin_data[0]).await;
                let latest_version = re
                    .captures(&latest)
                    .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                    .unwrap_or("-".to_string());
                result.insert("Latest".to_string(), latest_version);
            }
        }
        results.push(result);
    }
    results
}

// Downloads missing binaries and returns their status
async fn binary_get_missing(
    data: &HashMap<String, [String; 3]>,
) -> Result<String, Box<dyn std::error::Error>> {
    if !ensure_bin_directory() {
        return Ok("".to_string());
    }

    let xdg_bin_home = env::var("XDG_BIN_HOME").expect("XDG_BIN_HOME not set");
    let paths = fs::read_dir(&xdg_bin_home).expect("Failed to read XDG_BIN_HOME");
    let binaries: Vec<String> = paths
        .filter_map(|entry| {
            entry
                .ok()
                .map(|e| e.file_name().into_string().unwrap_or_default())
        })
        .collect();

    let not_found: Vec<String> = data
        .keys()
        .filter(|bin_name| !binaries.contains(bin_name))
        .cloned()
        .collect();

    if not_found.is_empty() {
        return Ok("All binaries are already present.".to_string());
    }

    for bin_name in not_found {
        println!("Downloading {}...", bin_name);
        binary_get(&bin_name, data).await?;
    }

    Ok("".to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let data = get_data();

    match cli.command {
        Some(Commands::Check { latest }) => {
            let results = binary_check(&data, latest).await;

            // Calculate maximum width for each column
            let mut max_binary = "Binary".len();
            let mut max_status = "Status".len();
            let mut max_version = "Version".len();
            let mut max_latest = if latest { "Latest".len() } else { 0 };

            for result in &results {
                max_binary = max_binary.max(result["Binary"].len());
                max_status = max_status.max(result["Status"].len());
                max_version = max_version.max(result["Version"].len());
                if latest {
                    max_latest = max_latest.max(result.get("Latest").map(|s| s.len()).unwrap_or(0));
                }
            }

            // Function to calculate visible string length (excluding ANSI codes)
            fn visible_length(s: &str) -> usize {
                let ansi_regex = regex::Regex::new(r"\x1B\[[0-9;]*m").unwrap();
                ansi_regex.replace_all(s, "").len()
            }

            // Print header
            if latest {
                println!(
                    "{:<width1$} {:<width2$} {:<width3$} {:<width4$}",
                    "Binary",
                    "Status",
                    "Version",
                    "Latest",
                    width1 = max_binary,
                    width2 = max_status,
                    width3 = max_version,
                    width4 = max_latest
                );
            } else {
                println!(
                    "{:<width1$} {:<width2$} {:<width3$}",
                    "Binary",
                    "Status",
                    "Version",
                    width1 = max_binary,
                    width2 = max_status,
                    width3 = max_version
                );
            }

            // Print rows
            for result in results {
                let status = result["Status"].as_str();
                let status_display = if status.contains("Found") {
                    format!("\x1b[32m{}\x1b[0m", status) // Green color
                } else {
                    format!("\x1b[31m{}\x1b[0m", status) // Red color
                };
                // Calculate padding for status column based on visible length
                let visible_status_len = visible_length(&status_display);
                let status_padding = max_status.saturating_sub(visible_status_len);

                if latest {
                    println!(
                        "{:<width1$}{:<width2$}{:<width3$}{:<width4$}",
                        result["Binary"].as_str(),
                        format!("{}{}", status_display, " ".repeat(status_padding)),
                        result["Version"].as_str(),
                        result.get("Latest").map(|s| s.as_str()).unwrap_or("-"),
                        width1 = max_binary,
                        width2 = max_status,
                        width3 = max_version,
                        width4 = max_latest
                    );
                } else {
                    println!(
                        "{:<width1$}{:<width2$}{:<width3$}",
                        result["Binary"].as_str(),
                        format!("{}{}", status_display, " ".repeat(status_padding)),
                        result["Version"].as_str(),
                        width1 = max_binary,
                        width2 = max_status,
                        width3 = max_version
                    );
                }
            }
        }
        Some(Commands::Get { bin_name }) => {
            binary_get(&bin_name, &data).await?;
        }
        Some(Commands::GetMissing) => {
            let missing_result = binary_get_missing(&data).await?;
            if !missing_result.is_empty() {
                println!("{}", missing_result);
            }
        }
        None => {
            // If no subcommand is provided and --version is not set, print help
            Cli::parse_from(&["bina", "--help"]);
        }
    }
    Ok(())
}
