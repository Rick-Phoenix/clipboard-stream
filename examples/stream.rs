#[cfg(not(target_os = "macos"))]
fn main() {}

use std::{
  fs::File,
  io::Write,
  sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
  },
};

use clipboard_stream::{Body, ClipboardEventListener, MimeType};
use futures::StreamExt;

#[cfg(target_os = "macos")]
#[tokio::main(flavor = "current_thread")]
async fn main() {
  let mut event_listener = ClipboardEventListener::spawn();
  let mut stream = event_listener.new_stream(32);
  let count = Arc::new(AtomicU32::new(0));

  while let Some(content) = stream.next().await {
    match content {
      Body::Utf8String(text) => {
        println!("got string: {}", text);
      }
      Body::Image {
        mime: mime_type,
        data: v,
      } => {
        println!("got: {:?}", mime_type);
        let cc = count.clone();
        tokio::task::spawn_blocking(move || {
          if mime_type == MimeType::ImagePng {
            let num = cc.fetch_add(1, Ordering::Relaxed);
            let file_name = format!("clip-img-{}.png", num);
            let mut file = File::create(file_name).unwrap();
            file.write_all(v.as_ref()).unwrap();
          }
        });
      }
    }
  }
}
