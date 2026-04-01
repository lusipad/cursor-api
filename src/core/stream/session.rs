// use super::decoder::StreamDecoder;
use crate::common::utils::parse_from_env;
use byte_str::ByteStr;
use bytes::Bytes;
use http_body_util::{
    BodyDataStream,
    combinators::{BoxBody, MapErr},
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::mpsc, task::JoinHandle};

// ===== Concrete types =====

type ResponseBody = BoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>;
type ResponseMapper = fn(Box<dyn std::error::Error + Send + Sync>) -> reqwest::Error;

pub type ByteStream = BodyDataStream<MapErr<ResponseBody, ResponseMapper>>;
pub type UpstreamTx = mpsc::Sender<Result<Bytes, std::io::Error>>;

// ===== Pending Tool Call =====

#[derive(Debug, Clone)]
pub struct PendingToolCall {
    pub id: ByteStr,
    pub name: ByteStr,
}

// ===== Bidi Session =====

pub struct BidiSession {
    pub upstream_tx: UpstreamTx,
    pub byte_stream: ByteStream,
    // pub decoder: StreamDecoder,
    // 当前只支持单工具调用
    pub pending_tool_calls: Vec<PendingToolCall>,
}

// ===== Session Key =====

pub fn session_key(ids: &mut [&str]) -> u64 {
    ids.sort_unstable();
    let len: usize = ids.iter().map(|s| s.len() + 1).sum();
    let mut buf = Vec::with_capacity(len);
    for id in ids.iter() {
        buf.extend_from_slice(id.as_bytes());
        buf.push(0xFF);
    }
    xxhash_rust::xxh3::xxh3_64(&buf)
}

// ===== Cache =====

struct Entry {
    session: BidiSession,
    timeout: JoinHandle<()>,
}

#[derive(Clone)]
pub struct SessionCache {
    inner: Arc<Inner>,
}

struct Inner {
    table: Mutex<HashMap<u64, Entry>>,
    timeout: Duration,
}

const DEFAULT: Duration = Duration::from_secs(300);

impl SessionCache {
    pub fn init() {
        let timeout = parse_from_env("TOOL_CALL_TIMEOUT", DEFAULT);
        use crate::core::adapter::{chat_completions, messages};
        chat_completions::SESSION_CACHE.init(Self::new(timeout));
        messages::SESSION_CACHE.init(Self::new(timeout));
    }

    fn new(timeout: Duration) -> Self {
        Self { inner: Arc::new(Inner { table: Mutex::new(HashMap::new()), timeout }) }
    }

    pub fn take(&self, key: u64) -> Option<BidiSession> {
        let entry = self.inner.table.lock().remove(&key)?;
        entry.timeout.abort();
        Some(entry.session)
    }

    pub fn park(&self, key: u64, session: BidiSession) {
        let inner = self.inner.clone();
        let dur = inner.timeout;
        let handle = tokio::spawn(async move {
            tokio::time::sleep(dur).await;
            inner.table.lock().remove(&key);
        });
        self.inner.table.lock().insert(key, Entry { session, timeout: handle });
    }

    // pub fn len(&self) -> usize { self.inner.table.lock().len() }
}
