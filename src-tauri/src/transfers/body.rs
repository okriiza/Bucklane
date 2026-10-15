use std::io::{Seek, SeekFrom};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll, ready};

use aws_sdk_s3::primitives::SdkBody;
use bytes::Bytes;
use futures_util::Stream;
use http_body::{Body, Frame, SizeHint};
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;

const CHUNK_SIZE: usize = 64 * 1024;

/// A request body that streams `len` bytes of a file starting at `offset`
/// and counts what has been sent in `counter`.
///
/// It is rebuildable, so the SDK can retry a failed request: each attempt
/// reopens the file. The counter is reset when an attempt *starts reading*,
/// not when its body is built, because the SDK may build the retry copy
/// before the first attempt runs; resetting early would double-count.
/// The file is never loaded into memory (TSD §1.2, streaming-first).
pub fn counting_file_body(
    path: PathBuf,
    offset: u64,
    len: u64,
    counter: Arc<AtomicU64>,
) -> SdkBody {
    SdkBody::retryable(move || {
        let body = match open_range(&path, offset) {
            Ok(file) => CountingBody {
                stream: Some(ReaderStream::with_capacity(file.take(len), CHUNK_SIZE)),
                error: None,
                started: false,
                counter: counter.clone(),
                len,
            },
            Err(err) => CountingBody {
                stream: None,
                error: Some(err),
                started: false,
                counter: counter.clone(),
                len,
            },
        };
        SdkBody::from_body_1_x(body)
    })
}

fn open_range(path: &std::path::Path, offset: u64) -> std::io::Result<tokio::fs::File> {
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    Ok(tokio::fs::File::from_std(file))
}

struct CountingBody {
    stream: Option<ReaderStream<tokio::io::Take<tokio::fs::File>>>,
    error: Option<std::io::Error>,
    /// Whether this attempt has begun; the first poll resets the counter.
    started: bool,
    counter: Arc<AtomicU64>,
    len: u64,
}

impl Body for CountingBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = self.get_mut();
        if !this.started {
            this.started = true;
            this.counter.store(0, Ordering::Relaxed);
        }
        if let Some(err) = this.error.take() {
            return Poll::Ready(Some(Err(err)));
        }
        let Some(stream) = this.stream.as_mut() else {
            return Poll::Ready(None);
        };
        match ready!(Pin::new(stream).poll_next(cx)) {
            Some(Ok(chunk)) => {
                this.counter
                    .fetch_add(chunk.len() as u64, Ordering::Relaxed);
                Poll::Ready(Some(Ok(Frame::data(chunk))))
            }
            Some(Err(err)) => Poll::Ready(Some(Err(err))),
            None => Poll::Ready(None),
        }
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_s3::primitives::ByteStream;

    #[tokio::test]
    async fn streams_the_requested_range_and_counts_bytes() {
        let path = std::env::temp_dir().join(format!("bucklane-body-{}", uuid::Uuid::now_v7()));
        std::fs::write(&path, b"0123456789").unwrap();
        let counter = Arc::new(AtomicU64::new(0));

        let body = counting_file_body(path.clone(), 2, 5, counter.clone());
        let bytes = ByteStream::new(body).collect().await.unwrap().into_bytes();

        assert_eq!(&bytes[..], b"23456");
        assert_eq!(counter.load(Ordering::Relaxed), 5);
        std::fs::remove_file(path).unwrap();
    }

    /// The retry copy is built before the first attempt reads anything, the
    /// way the SDK may do it; progress must still not be double-counted.
    #[tokio::test]
    async fn a_retry_does_not_double_count() {
        let path = std::env::temp_dir().join(format!("bucklane-body-{}", uuid::Uuid::now_v7()));
        std::fs::write(&path, b"abcdef").unwrap();
        let counter = Arc::new(AtomicU64::new(0));

        let body = counting_file_body(path.clone(), 0, 6, counter.clone());
        let retry = body.try_clone().expect("body must be retryable");
        ByteStream::new(body).collect().await.unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 6);
        ByteStream::new(retry).collect().await.unwrap();

        assert_eq!(
            counter.load(Ordering::Relaxed),
            6,
            "a retry must not double-count"
        );
        std::fs::remove_file(path).unwrap();
    }
}
