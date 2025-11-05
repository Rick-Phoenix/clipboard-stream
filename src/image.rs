use std::{io::Cursor, path::Path};

use image::{DynamicImage, ImageFormat, codecs::bmp::BmpDecoder};

pub(crate) fn convert_dib_to_png(dib_bytes: &[u8]) -> Option<Vec<u8>> {
  let cursor = Cursor::new(dib_bytes);

  let decoder = BmpDecoder::new_without_file_header(cursor).ok()?;

  let dynamic_image = DynamicImage::from_decoder(decoder).ok()?;

  let mut png_buffer = Vec::new();
  if dynamic_image
    .write_to(&mut Cursor::new(&mut png_buffer), ImageFormat::Png)
    .is_ok()
  {
    Some(png_buffer)
  } else {
    None
  }
}

const IMAGE_FORMATS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico"];

pub(crate) fn file_is_image(path: &Path) -> bool {
  path
    .extension()
    .is_some_and(|e| IMAGE_FORMATS.contains(&e.to_string_lossy().as_ref()))
}
