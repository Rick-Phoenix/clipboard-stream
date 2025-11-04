use std::{
  collections::HashMap,
  path::PathBuf,
  sync::{Arc, Mutex},
};

use futures::channel::mpsc::Sender;

use crate::mime_type::ImageMimeType;
use crate::{error::ClipboardResult, stream::StreamId};

/// Various kind of clipboard items.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Body {
  Html(String),
  PlainText(String),
  RichText(String),
  Image(ClipboardImage),
  FileList(Vec<PathBuf>),
  Custom { name: Arc<str>, data: Vec<u8> },
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClipboardImage {
  pub mime: ImageMimeType,
  pub bytes: Vec<u8>,
  pub path: Option<PathBuf>,
}

impl ClipboardImage {
  pub fn has_path(&self) -> bool {
    self.path.is_some()
  }
}

#[derive(Debug)]
pub(crate) struct BodySenders {
  senders: Mutex<HashMap<StreamId, Sender<ClipboardResult>>>,
}

impl BodySenders {
  pub(crate) fn new() -> Self {
    BodySenders {
      senders: Mutex::default(),
    }
  }

  /// Register Sender that was specified [`StreamId`].
  pub(crate) fn register(&self, id: StreamId, tx: Sender<ClipboardResult>) {
    let mut guard = self.senders.lock().unwrap();
    guard.insert(id, tx);
  }

  /// Close channel and unregister sender that was specified [`StreamId`]
  fn unregister(&self, id: &StreamId) {
    let mut guard = self.senders.lock().unwrap();
    guard.remove(id);
  }

  pub(crate) fn send_all(&self, result: ClipboardResult) {
    let mut senders = self.senders.lock().unwrap();

    for sender in senders.values_mut() {
      match sender.try_send(result.clone()) {
        Ok(_) => {}
        Err(e) => eprintln!("An error occurred while trying to send the clipboard data: {e}"),
      };
    }
  }
}

/// Handler for Cleaning up buffer(channel).
///
/// Close channel and unregister a specified [`StreamId`] of sender.
#[derive(Debug)]
pub(crate) struct BodySendersDropHandle(Arc<BodySenders>);

impl BodySendersDropHandle {
  pub(crate) fn new(senders: Arc<BodySenders>) -> Self {
    BodySendersDropHandle(senders)
  }

  pub(crate) fn drop(&self, id: &StreamId) {
    self.0.unregister(id);
  }
}
