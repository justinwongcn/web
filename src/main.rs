use futures::future::join_all;
use reqwest;

mod config;
mod logger;
mod service;

use config::Config;
use logger::FileLogger;
use service::CheckinService;

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