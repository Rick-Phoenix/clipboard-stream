use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};

use crate::body::BodySenders;
#[cfg(windows)]
use crate::error::ClipboardError;
#[cfg(target_os = "macos")]
use crate::sys::macos::OSXSys;

/// A trait observing clipboard change event and send data to receiver([`ClipboardStream`])
pub(super) trait Observer {
  fn observe(&mut self, body_senders: Arc<BodySenders>);
}

/// Observer for MacOS
#[cfg(target_os = "macos")]
pub(super) struct OSXObserver {
  stop: Arc<AtomicBool>,
  sys: OSXSys,
}

#[cfg(target_os = "macos")]
mod macos {
  impl OSXObserver {
    pub(super) fn new(stop: Arc<AtomicBool>) -> Self {
      OSXObserver {
        stop,
        sys: OSXSys::new(),
      }
    }
  }

  impl Observer for OSXObserver {
    fn observe(&mut self, body_senders: Arc<BodySenders>) {
      let mut last_count = self.sys.get_change_count();

      while !self.stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let change_count = self.sys.get_change_count();

        if change_count == last_count {
          continue;
        }
        last_count = change_count;

        self
          .sys
          .get_bodies()
          .into_iter()
          .for_each(|body| body_senders.send_all(Ok(Arc::new(body))));
      }
    }
  }
}

#[cfg(windows)]
pub(super) struct WinObserver {
  stop: Arc<AtomicBool>,
  pub(super) monitor: clipboard_win::Monitor,
}

#[cfg(windows)]
impl WinObserver {
  pub(super) fn new(stop: Arc<AtomicBool>, monitor: clipboard_win::Monitor) -> Self {
    WinObserver { stop, monitor }
  }

  pub(super) fn extract_image_bytes() -> Option<Vec<u8>> {
    use clipboard_win::{formats, Getter};

    let mut image_bytes: Vec<u8> = Vec::new();
    if let Ok(_num_bytes) = formats::Bitmap.read_clipboard(&mut image_bytes) {
      Some(image_bytes)
    } else {
      None
    }
  }

  pub(super) fn get_clipboard_content() -> Result<Vec<crate::Body>, ClipboardError> {
    use std::path::PathBuf;

    use clipboard_win::{formats, Clipboard, Getter};

    use crate::{body::ClipboardImage, Body};

    let mut bodies: Vec<Body> = Vec::with_capacity(1);

    let _clipboard =
      Clipboard::new_attempts(10).map_err(|e| ClipboardError::ReadError(e.to_string()))?;

    let mut file_list: Vec<PathBuf> = Vec::new();
    if let Ok(num_files) = formats::FileList.read_clipboard(&mut file_list) {
      if num_files == 1
        && file_list[0].extension().is_some_and(|ext| {
          let image_extensions = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "svg", "ico"];
          image_extensions.contains(&ext.to_string_lossy().as_ref())
        })
      {
        let image_path = file_list.remove(0);

        let bytes = if let Some(bytes_from_clipboard) = Self::extract_image_bytes() {
          Some(bytes_from_clipboard)
        } else {
          // Rare case, we try to read the bytes ourselves
          std::fs::read(&image_path).ok()
        };

        if let Some(bytes) = bytes {
          use crate::MimeType;

          bodies.push(Body::Image(ClipboardImage {
            bytes,
            path: Some(image_path),
            // Map this more accurately
            mime: MimeType::ImagePng,
          }));
        } else {
          // In the rare case that we have no bytes at all, we pass it as a normal path
          bodies.push(Body::FileList(file_list));
        }
      } else {
        bodies.push(Body::FileList(file_list));
      }
    } else if let Some(image_bytes) = Self::extract_image_bytes() {
      // Rare scenario, image bytes with no path

      use crate::MimeType;
      bodies.push(Body::Image(ClipboardImage {
        bytes: image_bytes,
        path: None,
        // Map this more accurately
        mime: MimeType::ImagePng,
      }));
    } else {
      // Falling back to a string
      let mut text = String::new();
      if let Ok(_num_bytes) = formats::Unicode.read_clipboard(&mut text) {
        bodies.push(Body::Utf8String(text));
      }
    }

    Ok(bodies)
  }
}

#[cfg(windows)]
impl Observer for WinObserver {
  fn observe(&mut self, body_senders: Arc<BodySenders>) {
    while !self.stop.load(Ordering::Relaxed) {
      let monitor = &mut self.monitor;

      match monitor.try_recv() {
        Ok(true) => {
          match WinObserver::get_clipboard_content() {
            Ok(bodies) => {
              for body in bodies {
                body_senders.send_all(Ok(Arc::new(body)));
              }
            }
            Err(e) => body_senders.send_all(Err(e)),
          };
        }
        Ok(false) => {
          // No event, waiting
          std::thread::park_timeout(std::time::Duration::from_millis(200));
        }
        Err(e) => {
          body_senders.send_all(Err(ClipboardError::MonitorFailed(e.to_string())));
          // Irrecoverable error, end the stream
          break;
        }
      }
    }
  }
}
