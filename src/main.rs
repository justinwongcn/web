use std::{io::Write, path::PathBuf, fs};

use futures::future::join_all;
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};
use serde_yaml;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Account {
    email: String,
    cookie: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    accounts: Vec<Account>,
    max_retries: u32,
    retry_delay: u64,
    log_file: String,
}

trait Logger {
    fn log(&self, content: &str) -> std::io::Result<()>;
}

struct FileLogger {
    file_path: PathBuf,
}

impl FileLogger {
    fn new(file_path: impl Into<PathBuf>) -> Self {
        Self {
            file_path: file_path.into(),
        }
    }
}

impl Logger for FileLogger {
    fn log(&self, content: &str) -> std::io::Result<()> {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .and_then(|mut file| file.write_all(format!("{}{}", content, "\n").as_bytes()))
    }
}

struct CheckinService {
    client: reqwest::Client,
    logger: Box<dyn Logger>,
    max_retries: u32,
    retry_delay: u64,
}

impl CheckinService {
    fn new(client: reqwest::Client, logger: Box<dyn Logger>, max_retries: u32, retry_delay: u64) -> Self {
        Self {
            client,
            logger,
            max_retries,
            retry_delay,
        }
    }

    async fn checkin(&self, account: &Account) -> Result<(), Box<dyn std::error::Error>> {
        let mut retries = 0;
        loop {
            match self.try_checkin(account).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    retries += 1;
                    if retries >= self.max_retries {
                        let error_log = format!("[{}] 账户 {} 签到失败 (重试{}次后): {}",
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                            account.email, retries, e);
                        eprintln!("{}", error_log);
                        self.logger.log(&error_log)?;
                        return Err(e);
                    }
                    sleep(Duration::from_secs(self.retry_delay)).await;
                }
            }
        }
    }

    async fn try_checkin(&self, account: &Account) -> Result<(), Box<dyn std::error::Error>> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("cookie", account.cookie.parse()?);

        let data = r#"{
    "token": "glados.one"
}"#;

        let json: serde_json::Value = serde_json::from_str(&data)?;

        let request = self.client.request(reqwest::Method::POST, "https://glados.rocks/api/user/checkin")
            .headers(headers)
            .json(&json);

        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;

        let response_json: serde_json::Value = match serde_json::from_str(&body) {
            Ok(json) => json,
            Err(e) => {
                return Err(format!("响应解析失败: {}\n响应内容: {}", e, body).into());
            }
        };
        
        if response_json["code"].as_i64().unwrap_or(0) == 1 {
            let message = response_json["message"].as_str().unwrap_or("No message");
            
            if let Some(first_item) = response_json["list"].as_array().and_then(|arr| arr.first()) {
                let change = first_item["change"].as_str().unwrap_or("0").split('.').next().unwrap_or("0");
                let balance = first_item["balance"].as_str().unwrap_or("0").split('.').next().unwrap_or("0");
                
                let log_content = format!("[{}] Account: {}, Message: {}, Change: {}, Balance: {}", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    account.email, message, change, balance);
                
                println!("{}", log_content);
                self.logger.log(&log_content)?
            }
        } else {
            let error_message = response_json["message"].as_str().unwrap_or("未知错误");
            return Err(format!("签到失败 - HTTP状态码: {}, 错误信息: {}", status, error_message).into());
        }

        Ok(())
    }
}

impl Config {
    fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load_from_file("config.yaml")?;

    let client = reqwest::Client::builder().build()?;
    let logger = Box::new(FileLogger::new(&config.log_file));
    let service = CheckinService::new(
        client,
        logger,
        config.max_retries,
        config.retry_delay,
    );

    let futures = config.accounts.into_iter().map(|account| {
        let service = &service;
        async move {
            let result = service.checkin(&account).await;
            match result {
                Ok(_) => (),
                Err(e) => {
                    let error_log = format!("[{}] 账户 {} 处理失败: {}", 
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        account.email, e);
                    eprintln!("{}", error_log);
                    if let Err(log_err) = service.logger.log(&error_log) {
                        eprintln!("记录日志失败: {}", log_err);
                    }
                }
            }
        }
    });

    join_all(futures).await;

    Ok(())
}