use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::OwnedSemaphorePermit;
use tokio_util::io::ReaderStream;
pub(super) struct OwnedStream {
    pub reader: ReaderStream<tokio::io::Take<tokio::fs::File>>,
    pub _permit: OwnedSemaphorePermit,
}
impl tokio_stream::Stream for OwnedStream {
    type Item = Result<axum::body::Bytes, std::io::Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().reader).poll_next(cx)
    }
}
