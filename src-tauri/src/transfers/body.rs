use std::io::{Seek, SeekFrom};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, ready};

use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::SdkBody;
use bytes::Bytes;
use futures_util::{Stream, TryStreamExt, stream};
use http_body::{Body, Frame, SizeHint};
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;

use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_provider::map_sdk_error;

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
            Ok(file) => CountingBody::new(
                ReaderStream::with_capacity(file.take(len), CHUNK_SIZE),
                counter.clone(),
                len,
            ),
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

/// A byte range of a stored object, pinned to the version with `etag`.
#[derive(Clone)]
pub struct ObjectRange {
    pub client: Client,
    pub bucket: String,
    pub key: String,
    pub etag: Option<String>,
    pub offset: u64,
    pub len: u64,
    /// Whole object size; a range covering all of it needs no Range support.
    pub size: u64,
}

/// A request body that streams an object range from another bucket or
/// connection and counts what has been sent in `counter`.
///
/// Like [`counting_file_body`] it is rebuildable: each attempt sends a new
/// ranged GET. Source failures are recorded in `failure`, because the SDK
/// reports a failing request body only as a generic send error.
pub fn counting_object_body(
    range: ObjectRange,
    counter: Arc<AtomicU64>,
    failure: Arc<Mutex<Option<AppError>>>,
) -> SdkBody {
    SdkBody::retryable(move || {
        let (range, failure) = (range.clone(), failure.clone());
        let len = range.len;
        let fetch = async move {
            let result = fetch_range(&range).await;
            if let Err(err) = &result {
                *failure.lock().expect("copy failure poisoned") = Some(err.clone());
            }
            result.map_err(std::io::Error::other)
        };
        SdkBody::from_body_1_x(CountingBody::new(
            stream::once(fetch).try_flatten(),
            counter.clone(),
            len,
        ))
    })
}

/// Opens a ranged GET and yields exactly `range.len` bytes of it.
async fn fetch_range(
    range: &ObjectRange,
) -> Result<impl Stream<Item = std::io::Result<Bytes>> + Send + use<>, AppError> {
    let body = if range.len == 0 {
        None
    } else {
        let output = range
            .client
            .get_object()
            .bucket(&range.bucket)
            .key(&range.key)
            .set_if_match(range.etag.clone())
            .range(format!(
                "bytes={}-{}",
                range.offset,
                range.offset + range.len - 1
            ))
            .send()
            .await
            .map_err(|e| map_sdk_error(e, ErrorCode::ObjectNotFound))?;
        // Without Range support the server sends the object from its start,
        // which would put the wrong bytes into this part.
        if output.content_range().is_none() && range.len != range.size {
            return Err(AppError::new(
                ErrorCode::OperationUnsupported,
                "The source storage ignored a byte-range request, so this large file can't be copied.",
            ));
        }
        Some(output.body)
    };

    Ok(stream::try_unfold(
        (body, range.len),
        |(body, remaining)| async move {
            let Some(mut body) = body.filter(|_| remaining > 0) else {
                return Ok(None);
            };
            match body.try_next().await.map_err(std::io::Error::other)? {
                Some(mut chunk) => {
                    chunk.truncate(usize::try_from(remaining).unwrap_or(usize::MAX));
                    let left = remaining - chunk.len() as u64;
                    Ok(Some((chunk, (Some(body), left))))
                }
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "The source ended before the expected size.",
                )),
            }
        },
    ))
}

fn open_range(path: &std::path::Path, offset: u64) -> std::io::Result<tokio::fs::File> {
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    Ok(tokio::fs::File::from_std(file))
}

type ByteChunks = Pin<Box<dyn Stream<Item = std::io::Result<Bytes>> + Send>>;

struct CountingBody {
    /// The mutex only makes the body `Sync`, as the SDK requires; polling
    /// has exclusive access and never locks.
    stream: Option<Mutex<ByteChunks>>,
    error: Option<std::io::Error>,
    /// Whether this attempt has begun; the first poll resets the counter.
    started: bool,
    counter: Arc<AtomicU64>,
    len: u64,
}

impl CountingBody {
    fn new(
        stream: impl Stream<Item = std::io::Result<Bytes>> + Send + 'static,
        counter: Arc<AtomicU64>,
        len: u64,
    ) -> Self {
        Self {
            stream: Some(Mutex::new(Box::pin(stream))),
            error: None,
            started: false,
            counter,
            len,
        }
    }
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
        let stream = stream.get_mut().expect("body stream poisoned");
        match ready!(stream.as_mut().poll_next(cx)) {
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
