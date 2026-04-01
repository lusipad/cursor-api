use alloc::sync::Arc;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use futures_core::stream::Stream;
use parking_lot::Mutex;
use tokio::sync::Notify;

/// 可通过外部信号控制的 Stream 包装器
/// 停止时将内部流存入 stash 而非 drop，支持取回复用
pub struct DroppableStream<S> {
    stream: Option<S>,
    stash: Arc<Mutex<Option<S>>>,
    notify: Arc<Notify>,
    dropped: bool,
}

/// 控制句柄
pub struct DropHandle<S> {
    notify: Arc<Notify>,
    stash: Arc<Mutex<Option<S>>>,
}

// S 不需要 Clone，Arc 内部共享
impl<S> Clone for DropHandle<S> {
    fn clone(&self) -> Self { Self { notify: self.notify.clone(), stash: self.stash.clone() } }
}

impl<S: Stream + Unpin> DroppableStream<S> {
    pub fn new(stream: S) -> (Self, DropHandle<S>) {
        let notify = Arc::new(Notify::new());
        let stash = Arc::new(Mutex::new(None));
        (
            Self {
                stream: Some(stream),
                stash: stash.clone(),
                notify: notify.clone(),
                dropped: false,
            },
            DropHandle { notify, stash },
        )
    }
}

impl<S: Stream + Unpin> Stream for DroppableStream<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.dropped {
            return Poll::Ready(None);
        }

        let notified = this.notify.notified();
        futures_util::pin_mut!(notified);

        if notified.poll(cx).is_ready() {
            // 存入 stash 而非 drop，支持后续 take_stream 取回
            *this.stash.lock() = this.stream.take();
            this.dropped = true;
            return Poll::Ready(None);
        }

        if let Some(ref mut stream) = this.stream {
            Pin::new(stream).poll_next(cx)
        } else {
            Poll::Ready(None)
        }
    }
}

impl<S> DropHandle<S> {
    /// 触发停止（流结束 / 正常关闭）
    pub fn drop_stream(self) { self.notify.notify_one() }

    /// 触发停止并取回内部流（用于 park）
    /// 必须在 DroppableStream 返回 None 之后调用（如 .chain() 中）
    pub fn take_stream(self) -> Option<S> {
        self.notify.notify_one();
        self.stash.lock().take()
    }
}
