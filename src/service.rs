use crate::{config::Account, logger::Logger};
use chrono;
use reqwest;
use serde_json;
use tokio::time::{sleep, Duration};

pub struct CheckinService {
    client: reqwest::Client,
    pub logger: Box<dyn Logger>,
    max_retries: u32,
    retry_delay: u64,
}

impl CheckinService {
    pub fn new(client: reqwest::Client, logger: Box<dyn Logger>, max_retries: u32, retry_delay: u64) -> Self {
        Self {
            client,
            logger,
            max_retries,
            retry_delay,
        }
    }

    pub async fn checkin(&self, account: &Account) -> Result<(), Box<dyn std::error::Error>> {
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