use std::{
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
  thread::JoinHandle,
};

use crate::{body::BodySenders, driver::observer::Observer, error::ClipboardError};

mod observer;

/// An event driver that monitors clipboard updates and notify
#[derive(Debug)]
pub(crate) struct Driver {
  stop: Arc<AtomicBool>,
  handle: Option<JoinHandle<()>>,
}

#[cfg(windows)]
impl Driver {
  /// Construct [`Driver`] and spawn a thread for monitoring clipboard events
  pub(crate) fn new(body_senders: Arc<BodySenders>) -> Result<Self, ClipboardError> {
    use std::sync::mpsc;

    let stop = Arc::new(AtomicBool::new(false));

    let stop_cl = stop.clone();

    let (init_tx, init_rx) = mpsc::sync_channel(0);

    // spawn OS thread
    // observe clipboard change event and send item
    let handle = std::thread::spawn(move || {
      match clipboard_win::Monitor::new() {
        Ok(monitor) => {
          init_tx.send(Ok(())).unwrap();

          let mut observer = observer::WinObserver::new(stop_cl, monitor);

          // event change observe loop
          observer.observe(body_senders);
        }
        Err(e) => {
          init_tx.send(Err(e)).unwrap();
        }
      };
    });

    // Block until we get an init signal
    match init_rx.recv() {
      Ok(Ok(())) => Ok(Driver {
        stop,
        handle: Some(handle),
      }),
      Ok(Err(e)) => Err(ClipboardError::InitializationError(format!("{e:#?}"))),
      Err(e) => Err(ClipboardError::TryRecvError(e.to_string())),
    }
  }
}

#[cfg(target_os = "macos")]
impl Driver {
  /// Construct [`Driver`] and spawn a thread for monitoring clipboard events
  pub(crate) fn new(body_senders: Arc<BodySenders>) -> Result<Self, ClipboardError> {
    let stop = Arc::new(AtomicBool::new(false));

    let stop_cl = stop.clone();

    // spawn OS thread
    // observe clipboard change event and send item
    let handle = std::thread::spawn(move || {
      // construct Observer in thread
      // OSXSys is **not** implemented Send + Sync
      // in order to send Observer, construct it
      let mut observer = observer::OSXObserver::new(stop_cl);

      // event change observe loop
      observer.observe(body_senders);
    });

    Ok(Driver {
      stop,
      handle: Some(handle),
    })
  }
}

impl Drop for Driver {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::Relaxed);
    if let Some(handle) = self.handle.take() {
      handle.join().unwrap();
    }
  }
}
