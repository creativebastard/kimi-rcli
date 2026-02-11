//! Async stream abstractions for I/O operations.

use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, BufReader, ReadBuf};

/// A trait for types that can be read asynchronously.
///
/// This is a convenience trait that combines `AsyncRead` and `Unpin`.
pub trait AsyncReadable: AsyncRead + Unpin + Send {}

impl<T> AsyncReadable for T where T: AsyncRead + Unpin + Send {}

/// A trait for types that can be written asynchronously.
///
/// This is a convenience trait that combines `AsyncWrite` and `Unpin`.
pub trait AsyncWritable: AsyncWrite + Unpin + Send {}

impl<T> AsyncWritable for T where T: AsyncWrite + Unpin + Send {}

/// A line-based async reader.
///
/// `LineReader` wraps an async reader and provides line-by-line reading
/// capabilities. It uses a buffered reader internally for efficiency.
///
/// # Examples
///
/// ```
/// use kaos_rs::LineReader;
/// use tokio::io::AsyncBufReadExt;
///
/// # async fn example() -> std::io::Result<()> {
/// let data = b"line1\nline2\nline3";
/// let reader = LineReader::new(&data[..]);
/// let mut lines = reader.lines();
///
/// while let Some(line) = lines.next_line().await? {
///     println!("{}", line);
/// }
/// # Ok(())
/// # }
/// ```
pub struct LineReader<R> {
    inner: BufReader<R>,
}

impl<R: AsyncRead + Unpin> LineReader<R> {
    /// Creates a new `LineReader` from an async reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::LineReader;
    ///
    /// let data: &[u8] = b"hello\nworld";
    /// let reader = LineReader::new(data);
    /// ```
    pub fn new(reader: R) -> Self {
        Self {
            inner: BufReader::new(reader),
        }
    }

    /// Creates a new `LineReader` with a specific buffer capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::LineReader;
    ///
    /// let data: &[u8] = b"hello\nworld";
    /// let reader = LineReader::with_capacity(1024, data);
    /// ```
    pub fn with_capacity(capacity: usize, reader: R) -> Self {
        Self {
            inner: BufReader::with_capacity(capacity, reader),
        }
    }

    /// Returns a reference to the underlying reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::LineReader;
    ///
    /// let data: &[u8] = b"hello";
    /// let reader = LineReader::new(data);
    /// let underlying = reader.get_ref();
    /// ```
    pub fn get_ref(&self) -> &R {
        self.inner.get_ref()
    }

    /// Returns a mutable reference to the underlying reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::LineReader;
    ///
    /// let data: &[u8] = b"hello";
    /// let mut reader = LineReader::new(data);
    /// let underlying = reader.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> &mut R {
        self.inner.get_mut()
    }

    /// Consumes the `LineReader` and returns the underlying reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::LineReader;
    ///
    /// let data: &[u8] = b"hello";
    /// let reader = LineReader::new(data);
    /// let underlying = reader.into_inner();
    /// ```
    pub fn into_inner(self) -> R {
        self.inner.into_inner()
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for LineReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<R: AsyncRead + Unpin> AsyncBufRead for LineReader<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<&[u8]>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_fill_buf(cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut self.inner).consume(amt)
    }
}

/// An async writer that counts the number of bytes written.
///
/// # Examples
///
/// ```
/// use kaos_rs::CountingWriter;
/// use tokio::io::AsyncWriteExt;
///
/// # async fn example() -> std::io::Result<()> {
/// let mut buf = Vec::new();
/// let mut writer = CountingWriter::new(&mut buf);
/// writer.write_all(b"hello world").await?;
/// assert_eq!(writer.bytes_written(), 11);
/// # Ok(())
/// # }
/// ```
pub struct CountingWriter<W> {
    inner: W,
    count: u64,
}

impl<W: AsyncWrite + Unpin> CountingWriter<W> {
    /// Creates a new `CountingWriter` wrapping the given writer.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::CountingWriter;
    ///
    /// let buf = Vec::new();
    /// let writer = CountingWriter::new(buf);
    /// ```
    pub fn new(writer: W) -> Self {
        Self {
            inner: writer,
            count: 0,
        }
    }

    /// Returns the number of bytes written so far.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::CountingWriter;
    ///
    /// let buf = Vec::new();
    /// let writer = CountingWriter::new(buf);
    /// assert_eq!(writer.bytes_written(), 0);
    /// ```
    pub fn bytes_written(&self) -> u64 {
        self.count
    }

    /// Returns a reference to the underlying writer.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::CountingWriter;
    ///
    /// let buf = Vec::new();
    /// let writer = CountingWriter::new(buf);
    /// let underlying = writer.get_ref();
    /// ```
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Returns a mutable reference to the underlying writer.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::CountingWriter;
    ///
    /// let buf = Vec::new();
    /// let mut writer = CountingWriter::new(buf);
    /// let underlying = writer.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Consumes the `CountingWriter` and returns the underlying writer.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::CountingWriter;
    ///
    /// let buf = Vec::new();
    /// let writer = CountingWriter::new(buf);
    /// let underlying = writer.into_inner();
    /// ```
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for CountingWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match Pin::new(&mut self.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(n)) => {
                self.count += n as u64;
                Poll::Ready(Ok(n))
            }
            other => other,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Extension trait for async readers to provide line-based reading.
///
/// This trait provides convenient methods for reading lines from async readers.
///
/// # Examples
///
/// ```
/// use kaos_rs::StreamExt;
/// use tokio::io::AsyncBufReadExt;
///
/// # async fn example() -> std::io::Result<()> {
/// let data = b"line1\nline2\nline3";
/// let reader = kaos_rs::LineReader::new(&data[..]);
/// let mut lines = StreamExt::lines(reader);
///
/// while let Some(line) = lines.next_line().await? {
///     println!("{}", line);
/// }
/// # Ok(())
/// # }
/// ```
pub trait StreamExt: AsyncBufRead + Unpin + Sized {
    /// Creates a stream of lines from this reader.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaos_rs::{LineReader, StreamExt};
    /// use tokio::io::AsyncBufReadExt;
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// let data = b"line1\nline2\nline3";
    /// let reader = LineReader::new(&data[..]);
    /// let mut lines = StreamExt::lines(reader);
    ///
    /// while let Some(line) = lines.next_line().await? {
    ///     println!("{}", line);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn lines(self) -> tokio::io::Lines<Self> {
        tokio::io::AsyncBufReadExt::lines(self)
    }
}

impl<T: AsyncBufRead + Unpin + Sized> StreamExt for T {}

/// Re-export of `tokio::io::AsyncReadExt` for convenience.
pub use tokio::io::AsyncReadExt;

/// Re-export of `tokio::io::AsyncWriteExt` for convenience.
pub use tokio::io::AsyncWriteExt;

/// Re-export of `tokio::io::AsyncBufReadExt` for convenience.
pub use tokio::io::AsyncBufReadExt;
