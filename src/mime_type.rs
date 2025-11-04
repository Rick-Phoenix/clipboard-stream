use std::ffi::OsStr;
use std::fmt;
use std::str::FromStr;

/// Represents a specific, known image MIME type for use within the application.
///
/// This provides a type-safe way to handle image formats, avoiding "stringly-typed" APIs.
/// It includes an `Unknown` variant to gracefully handle any unrecognized MIME types,
/// preserving the original string for inspection.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageMimeType {
  #[cfg_attr(feature = "serde", serde(rename = "image/png"))]
  Png,
  #[cfg_attr(feature = "serde", serde(rename = "image/jpeg"))]
  Jpeg,
  #[cfg_attr(feature = "serde", serde(rename = "image/gif"))]
  Gif,
  #[cfg_attr(feature = "serde", serde(rename = "image/webp"))]
  Webp,
  #[cfg_attr(feature = "serde", serde(rename = "image/bmp"))]
  Bmp,
  #[cfg_attr(feature = "serde", serde(rename = "image/svg+xml"))]
  Svg,
  #[cfg_attr(feature = "serde", serde(rename = "image/vnd.microsoft.icon"))]
  Ico,
  /// A fallback for any MIME type that is not explicitly known.
  /// It stores the original, unrecognized string.
  #[cfg_attr(feature = "serde", serde(untagged))]
  Unknown(String),
}

impl ImageMimeType {
  pub fn from_ext(ext: &OsStr) -> Option<Self> {
    let value = match ext.to_string_lossy().as_ref() {
      "png" => Self::Png,
      "jpg" | "jpeg" => Self::Jpeg,
      "gif" => Self::Gif,
      "bmp" => Self::Bmp,
      "webp" => Self::Webp,
      "svg" => Self::Svg,
      "ico" => Self::Ico,
      _ => {
        return None;
      }
    };

    Some(value)
  }
  /// Creates an `ImageMimeType` from a string slice.
  ///
  /// This is the primary way to create an instance. It attempts to parse the
  /// string into a known variant, and if it fails, it gracefully falls back
  /// to the `Unknown` variant, capturing the original string.
  ///
  /// # Example
  /// ```
  /// let png = ImageMimeType::from_str("image/png");
  /// assert_eq!(png, ImageMimeType::Png);
  ///
  /// let unknown = ImageMimeType::from_str("application/pdf");
  /// assert_eq!(unknown, ImageMimeType::Unknown("application/pdf".to_string()));
  /// ```
  pub fn from_name(s: &str) -> Self {
    s.parse().unwrap_or_else(|_| Self::Unknown(s.to_string()))
  }
}

/// Allows the enum to be printed as its corresponding MIME string.
///
/// # Example
/// ```
/// let mime = ImageMimeType::Png;
/// println!("{}", mime); // "image/png"
/// ```
impl fmt::Display for ImageMimeType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ImageMimeType::Png => write!(f, "image/png"),
      ImageMimeType::Jpeg => write!(f, "image/jpeg"),
      ImageMimeType::Gif => write!(f, "image/gif"),
      ImageMimeType::Webp => write!(f, "image/webp"),
      ImageMimeType::Bmp => write!(f, "image/bmp"),
      ImageMimeType::Svg => write!(f, "image/svg+xml"),
      ImageMimeType::Ico => write!(f, "image/vnd.microsoft.icon"),
      ImageMimeType::Unknown(original) => write!(f, "{}", original),
    }
  }
}

/// A simple error type for the strict parsing implementation.
#[derive(Debug, PartialEq, Eq)]
pub struct ParseImageMimeTypeError;

/// Implements the standard Rust trait for parsing from a string.
impl FromStr for ImageMimeType {
  type Err = ParseImageMimeTypeError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "image/png" => Ok(ImageMimeType::Png),
      "image/jpeg" | "image/jpg" => Ok(ImageMimeType::Jpeg),
      "image/gif" => Ok(ImageMimeType::Gif),
      "image/webp" => Ok(ImageMimeType::Webp),
      "image/bmp" => Ok(ImageMimeType::Bmp),
      "image/svg+xml" => Ok(ImageMimeType::Svg),
      "image/x-icon" | "image/vnd.microsoft.icon" => Ok(ImageMimeType::Ico),
      _ => {
        eprintln!("Unknown mime type `{s}`");
        Err(ParseImageMimeTypeError)
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parsing_and_fallback() {
    // Test unknown type
    let unknown = ImageMimeType::from_name("application/pdf");
    assert_eq!(
      unknown,
      ImageMimeType::Unknown("application/pdf".to_string())
    );

    // Test empty string
    let empty = ImageMimeType::from_name("");
    assert_eq!(empty, ImageMimeType::Unknown("".to_string()));
  }

  #[test]
  fn test_display() {
    assert_eq!(ImageMimeType::Png.to_string(), "image/png");
    assert_eq!(ImageMimeType::Jpeg.to_string(), "image/jpeg");
    assert_eq!(ImageMimeType::Svg.to_string(), "image/svg+xml");
    let unknown = ImageMimeType::Unknown("video/mp4".to_string());
    assert_eq!(unknown.to_string(), "video/mp4");
  }

  #[test]
  fn test_strict_from_str() {
    assert_eq!("image/gif".parse::<ImageMimeType>(), Ok(ImageMimeType::Gif));
    assert_eq!(
      "text/plain".parse::<ImageMimeType>(),
      Err(ParseImageMimeTypeError)
    );
  }
}
