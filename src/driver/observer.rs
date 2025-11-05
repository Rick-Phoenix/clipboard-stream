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

#[cfg(windows)]
fn extract_clipboard_format(format_id: u32, max_size: Option<usize>) -> Option<Vec<u8>> {
  use clipboard_win::formats;

  // We check if the format is available at all
  clipboard_win::size(format_id)
    // Then, whether the size is within the allowed range
    .filter(|size| max_size.is_none_or(|max| max > size.get()))
    // Then, if the data can be read
    .and_then(|_| clipboard_win::get(formats::RawData(format_id)).ok())
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
  max_image_bytes: Option<usize>,
  max_bytes: Option<usize>,
}

#[cfg(windows)]
impl WinObserver {
  pub(super) fn new(
    stop: Arc<AtomicBool>,
    monitor: clipboard_win::Monitor,
    custom_formats: Vec<Arc<str>>,
    interval: Option<Duration>,
    max_image_bytes: Option<usize>,
    max_bytes: Option<usize>,
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

    let max_image_bytes = if max_image_bytes.is_none() && max_bytes.is_some() {
      max_bytes
    } else {
      max_image_bytes
    };

    WinObserver {
      stop,
      monitor,
      html_format,
      png_format,
      rtf_format,
      custom_formats: custom_formats_map,
      interval: interval.unwrap_or_else(|| Duration::from_millis(200)),
      max_image_bytes,
      max_bytes,
    }
  }

  pub(super) fn extract_image_bytes(&self) -> Option<Vec<u8>> {
    use clipboard_win::formats;

    use crate::image::convert_dib_to_png;

    let max_image_bytes = self.max_image_bytes;

    if let Some(png_code) = self.png_format
      && let Some(png_bytes) = extract_clipboard_format(png_code.get(), max_image_bytes)
    {
      Some(png_bytes)
    } else if let Some(bytes) = extract_clipboard_format(formats::CF_DIBV5, max_image_bytes)
      && let Some(png_bytes) = convert_dib_to_png(&bytes)
    {
      Some(png_bytes)
    } else if let Some(bytes) = extract_clipboard_format(formats::CF_DIB, max_image_bytes)
      && let Some(png_bytes) = convert_dib_to_png(&bytes)
    {
      Some(png_bytes)
    } else {
      None
    }
  }

  pub(super) fn extract_files_list(max_size: Option<usize>) -> Option<Vec<PathBuf>> {
    use clipboard_win::{formats, Getter};

    clipboard_win::size(formats::FileList.into())
      .filter(|size| max_size.is_none_or(|max| max > size.get()))
      .and_then(|_| {
        let mut files_list: Vec<PathBuf> = Vec::new();
        if let Ok(_num_files) = formats::FileList.read_clipboard(&mut files_list) {
          Some(files_list)
        } else {
          None
        }
      })
  }

  pub(super) fn get_clipboard_content(&self) -> Result<crate::Body, ClipboardError> {
    use clipboard_win::{formats, Clipboard, Getter};

    use crate::{body::ClipboardImage, Body};

    let _clipboard =
      Clipboard::new_attempts(10).map_err(|e| ClipboardError::ReadError(e.to_string()))?;

    let max_bytes = self.max_bytes;

    for (name, id) in self.custom_formats.iter() {
      if let Some(bytes) = extract_clipboard_format(id.get(), max_bytes) {
        return Ok(Body::Custom {
          name: name.clone(),
          data: bytes,
        });
      }
    }

    if let Some(image_bytes) = self.extract_image_bytes() {
      let image_path = if let Some(mut files_list) = Self::extract_files_list(max_bytes)
        && files_list.len() == 1
      {
        Some(files_list.remove(0))
      } else {
        None
      };

      Ok(Body::Image(ClipboardImage {
        bytes: image_bytes,
        path: image_path,
      }))
    } else if let Some(mut files_list) = Self::extract_files_list(max_bytes) {
      // Trying to detect if the single file is just an image
      use crate::image::file_is_image;

      // We check if there is just one file
      if files_list.len() == 1
        && let Some(path) = files_list.first()

        // Then, if it's an image
        && file_is_image(path)
        // Then, if the size is within the allowed range
        && max_bytes.is_none_or(|max| path.metadata().is_ok_and(|metadata| max as u64 > metadata.len()))
        // Then, if the bytes are readable
        && let Ok(bytes) =  std::fs::read(path)
      //
      // Only if all of these are true, we save it as an image
      {
        let image_path = files_list.remove(0);

        Ok(Body::Image(ClipboardImage {
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
