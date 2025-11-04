use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};
#[cfg(windows)]
use std::{collections::HashMap, num::NonZeroU32, path::PathBuf, time::Duration};

use crate::body::BodySenders;
#[cfg(windows)]
use crate::error::ClipboardError;
#[cfg(windows)]
use crate::mime_type::ImageMimeType;
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
  monitor: clipboard_win::Monitor,
  html_format: Option<clipboard_win::formats::Html>,
  png_format: Option<NonZeroU32>,
  rtf_format: Option<NonZeroU32>,
  custom_formats: HashMap<Arc<str>, NonZeroU32>,
  interval: Duration,
}

#[cfg(windows)]
impl WinObserver {
  pub(super) fn new(
    stop: Arc<AtomicBool>,
    monitor: clipboard_win::Monitor,
    custom_formats: Vec<Arc<str>>,
    interval: Option<Duration>,
  ) -> Self {
    let html_format = clipboard_win::formats::Html::new();
    let png_format = clipboard_win::register_format("PNG");
    let rtf_format = clipboard_win::register_format("Rich Text Format");

    let custom_formats_map: HashMap<Arc<str>, NonZeroU32> = custom_formats
      .into_iter()
      .filter_map(|name| {
        if let Some(id) = clipboard_win::register_format(name.as_ref()) {
          Some((name, id))
        } else {
          eprintln!("[ ERROR ] Failed to register custom clipboard type `{name}`");
          None
        }
      })
      .collect();

    WinObserver {
      stop,
      monitor,
      html_format,
      png_format,
      rtf_format,
      custom_formats: custom_formats_map,
      interval: interval.unwrap_or_else(|| Duration::from_millis(200)),
    }
  }

  pub(super) fn extract_image_bytes(&self) -> Option<Vec<u8>> {
    use clipboard_win::formats;

    if let Some(png_code) = self.png_format
      && let Ok(bytes) = clipboard_win::get(formats::RawData(png_code.get()))
    {
      eprintln!("Found png");
      Some(bytes)
    } else if let Ok(bytes) = clipboard_win::get(formats::RawData(formats::CF_DIBV5)) {
      eprintln!("Found dibv5");
      Some(bytes)
    } else {
      clipboard_win::get(formats::RawData(formats::CF_DIB)).ok()
    }
  }

  pub(super) fn extract_files_list() -> Option<Vec<PathBuf>> {
    use clipboard_win::{formats, Getter};

    let mut files_list: Vec<PathBuf> = Vec::new();
    if let Ok(_num_files) = formats::FileList.read_clipboard(&mut files_list) {
      Some(files_list)
    } else {
      None
    }
  }

  pub(super) fn get_clipboard_content(&self) -> Result<crate::Body, ClipboardError> {
    use clipboard_win::{formats, Clipboard, Getter};

    use crate::{body::ClipboardImage, Body};

    let _clipboard =
      Clipboard::new_attempts(10).map_err(|e| ClipboardError::ReadError(e.to_string()))?;

    for (name, id) in self.custom_formats.iter() {
      if let Ok(bytes) = clipboard_win::get(formats::RawData(id.get())) {
        return Ok(Body::Custom {
          name: name.clone(),
          data: bytes,
        });
      }
    }

    if let Some(image_bytes) = self.extract_image_bytes() {
      let image_path = if let Some(mut files_list) = Self::extract_files_list()
        && files_list.len() == 1
      {
        Some(files_list.remove(0))
      } else {
        None
      };

      let mime_type = image_path
        .as_ref()
        .and_then(|path| {
          // We try with the extension first
          if let Some(ext) = path.extension() {
            ImageMimeType::from_ext(ext)
          } else {
            // Otherwise, we try reading the file
            mimetype_detector::detect_file(path)
              .ok()
              .map(|mime| ImageMimeType::from_name(mime.mime()))
          }
        })
        // Falling back to octet-stream in any case
        .unwrap_or_else(|| ImageMimeType::Unknown("application/octet-stream".to_string()));

      Ok(Body::Image(ClipboardImage {
        bytes: image_bytes,
        path: image_path,
        mime: mime_type,
      }))
    } else if let Some(mut files_list) = Self::extract_files_list() {
      // Trying to detect if the single file is just an image
      if files_list.len() == 1
        && let Some((bytes, mime)) = files_list
          .first()
          .unwrap()
          .extension()
          // Check if the extension is supported
          .and_then(ImageMimeType::from_ext)
          // If it is, try to read the bytes
          .and_then(|mime| {
            if let Ok(bytes) = std::fs::read(files_list.first().unwrap()) {
              Some((bytes, mime))
            } else {
              None
            }
          })
      {
        // Only if we have a valid mime type AND the bytes we proceed with the image

        let image_path = files_list.remove(0);

        Ok(Body::Image(ClipboardImage {
          mime,
          bytes,
          path: Some(image_path),
        }))
      } else {
        Ok(Body::FileList(files_list))
      }
    } else {
      let mut text = String::new();
      if let Some(html_parser) = self.html_format
        && let Ok(_) = html_parser.read_clipboard(&mut text)
      {
        Ok(Body::Html(text))
      } else if let Some(rtf_format) = self.rtf_format
        && let Ok(bytes) = clipboard_win::get(formats::RawData(rtf_format.get()))
      {
        Ok(Body::RichText(String::from_utf8_lossy(&bytes).to_string()))
      } else if let Ok(_num_bytes) = formats::Unicode.read_clipboard(&mut text) {
        Ok(Body::PlainText(text))
      } else {
        Err(ClipboardError::UnknownDataType)
      }
    }
  }
}

#[cfg(windows)]
impl Observer for WinObserver {
  fn observe(&mut self, body_senders: Arc<BodySenders>) {
    while !self.stop.load(Ordering::Relaxed) {
      let monitor = &mut self.monitor;

      match monitor.try_recv() {
        Ok(true) => {
          match self.get_clipboard_content() {
            Ok(body) => {
              body_senders.send_all(Ok(Arc::new(body)));
            }
            Err(e) => body_senders.send_all(Err(e)),
          };
        }
        Ok(false) => {
          // No event, waiting
          std::thread::sleep(self.interval);
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
