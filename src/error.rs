use std::sync::Arc;

use thiserror::Error;

use crate::Body;

#[derive(Clone, Debug, Error)]
pub enum ClipboardError {
  #[error("Failed to start clipboard monitor: {0}")]
  InitializationError(String),

  #[error("Failed to monitor the clipboard: {0}")]
  MonitorFailed(String),

  #[error("Failed to receive data from channel: {0}")]
  TryRecvError(String),

  #[error("Failed to read the clipboard: {0}")]
  ReadError(String),

  #[error("Failed to read the clipboard: unknown data type")]
  UnknownDataType,
}

pub type ClipboardResult = Result<Arc<Body>, ClipboardError>;
