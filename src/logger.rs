use std::{io::Write, path::PathBuf};

pub trait Logger {
    fn log(&self, content: &str) -> std::io::Result<()>;
}

pub struct FileLogger {
    file_path: PathBuf,
}

impl FileLogger {
    pub fn new(file_path: impl Into<PathBuf>) -> Self {
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