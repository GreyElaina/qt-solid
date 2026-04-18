use std::{fmt, sync::Arc};

pub type Result<T> = std::result::Result<T, QtWgpuRendererError>;

#[derive(Debug, Clone)]
pub struct QtWgpuRendererError {
    message: Arc<str>,
}

impl QtWgpuRendererError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
        }
    }
}

impl fmt::Display for QtWgpuRendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QtWgpuRendererError {}
