use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub email: String,
    pub cookie: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub accounts: Vec<Account>,
    pub max_retries: u32,
    pub retry_delay: u64,
    pub log_file: String,
}

impl Config {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content)?
        } else {
            serde_json::from_str(&content)?
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.accounts.is_empty() {
            return Err("No accounts configured".into());
        }
        if self.max_retries == 0 {
            return Err("max_retries must be greater than 0".into());
        }
        if self.log_file.is_empty() {
            return Err("log_file path must not be empty".into());
        }
        Ok(())
    }
}