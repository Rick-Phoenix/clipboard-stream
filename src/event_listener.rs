#[cfg(target_os = "macos")]
use std::convert::Infallible;
use std::sync::{
  atomic::{AtomicUsize, Ordering},
  Arc,
};

use futures::channel::mpsc;

#[cfg(windows)]
use crate::error::ClipboardError;
use crate::{
  body::{BodySenders, BodySendersDropHandle},
  driver::Driver,
  stream::StreamId,
  ClipboardStream,
};

/// Clipboard event change listener.
///
/// Listen for clipboard change events and notifies [`ClipboardStream`].
pub struct ClipboardEventListener {
  driver: Option<Driver>,
  body_senders: Arc<BodySenders>,
  id: AtomicUsize,
}

impl ClipboardEventListener {
  /// Creates a new [`ClipboardEventListener`] that monitors clipboard changes in a dedicated OS thread.
  pub fn spawn() -> Result<Self, ClipboardError> {
    let body_senders = Arc::new(BodySenders::new());

    let driver = Driver::new(body_senders.clone())?;
    Ok(ClipboardEventListener {
      driver: Some(driver),
      body_senders,
      id: AtomicUsize::new(0),
    })
  }

  /// Creates a [`ClipboardStream`] for receiving clipboard change items as [`Body`].
  ///
  /// # Buffer size
  /// This method takes a buffer size. Items are buffered when not received immediately.
  /// The actual buffer capacity is `buf_size + 2`, where the extra `2` accounts for the
  /// number of internal senders used by the library.
  ///
  /// # Example
  /// ```
  /// # use clipboard_stream::{Body, ClipboardEventListener, ClipboardStream};
  /// # #[tokio::main]
  /// # async fn main() {
  ///     let mut event_listener = ClipboardEventListener::spawn();
  ///
  ///     let buf_size = 32;
  ///     let stream = event_listener.new_stream(buf_size);
  /// # }
  /// ```
  /// [`Body`]: crate::Body
  pub fn new_stream(&mut self, buffer: usize) -> ClipboardStream {
    let (tx, rx) = mpsc::channel(buffer);
    let id = StreamId(self.id.fetch_add(1, Ordering::Relaxed));
    self.body_senders.register(id.clone(), tx);
    let drop_handle = BodySendersDropHandle::new(self.body_senders.clone());

    ClipboardStream {
      id,
      body_rx: Box::pin(rx),
      drop_handle,
    }
  }
}

#[cfg(target_os = "macos")]
impl Default for ClipboardEventListener {
  fn default() -> Self {
    ClipboardEventListener::spawn()
  }
}

impl Drop for ClipboardEventListener {
  fn drop(&mut self) {
    drop(self.driver.take())
  }
}
