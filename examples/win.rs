use clipboard_stream::{Body, ClipboardEventListener};
use futures::StreamExt;

#[tokio::main]
async fn main() {
  let mut event_listener = ClipboardEventListener::spawn().unwrap();

  let mut stream = event_listener.new_stream(32);

  while let Some(result) = stream.next().await {
    match result {
      Ok(content) => {
        match content.as_ref() {
          Body::Utf8String(v) => println!("got string: {}", v),
          Body::Image(image) => {
            println!("Received image");
            if let Some(path) = &image.path {
              println!("Image Path: {path:#?}");
            }
          }
          Body::FileList(files) => println!("Received files: {files:#?}"),
        };
      }
      Err(e) => eprintln!("{e}"),
    }
  }
}
