use xxhash_rust::xxh3::xxh3_128;

const FIELD_SEP: u8 = 0xFF;
const ITEM_SEP: u8 = 0xFE;

// ===== Buf =====

pub struct Buf {
    inner: Vec<u8>,
}

impl Buf {
    #[inline]
    pub fn new() -> Self { Self { inner: Vec::with_capacity(256) } }

    #[inline]
    pub fn clear(&mut self) { self.inner.clear() }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] { &self.inner }

    // ---- 原始写入 ----

    #[inline]
    pub fn put(&mut self, data: &[u8]) -> &mut Self {
        self.inner.extend_from_slice(data);
        self
    }

    #[inline]
    pub fn put_str(&mut self, s: &str) -> &mut Self { self.put(s.as_bytes()) }

    #[inline]
    pub fn put_u128_le(&mut self, v: u128) -> &mut Self { self.put(&v.to_le_bytes()) }

    #[inline]
    pub fn put_tag(&mut self, tag: u8) -> &mut Self {
        self.inner.push(tag);
        self
    }

    // ---- 作用域：无条件 ----

    /// 写入一个字段，闭包结束后自动追加 FIELD_SEP
    #[inline]
    pub fn field(&mut self, f: impl FnOnce(&mut Self)) -> &mut Self {
        f(self);
        self.inner.push(FIELD_SEP);
        self
    }

    /// 写入一个子项，闭包结束后自动追加 ITEM_SEP
    #[inline]
    pub fn item(&mut self, f: impl FnOnce(&mut Self)) -> &mut Self {
        f(self);
        self.inner.push(ITEM_SEP);
        self
    }

    // ---- 作用域：条件 ----

    /// 条件为 true 时写入字段
    #[inline]
    pub fn field_if(&mut self, condition: bool, f: impl FnOnce(&mut Self)) -> &mut Self {
        if condition {
            f(self);
            self.inner.push(FIELD_SEP);
        }
        self
    }

    /// 条件为 true 时写入子项
    #[inline]
    pub fn item_if(&mut self, condition: bool, f: impl FnOnce(&mut Self)) -> &mut Self {
        if condition {
            f(self);
            self.inner.push(ITEM_SEP);
        }
        self
    }

    // ---- 作用域：Optional ----

    /// Some 时写入字段，None 时跳过（不写分隔符）
    #[inline]
    pub fn field_opt<T>(&mut self, val: Option<T>, f: impl FnOnce(&mut Self, T)) -> &mut Self {
        if let Some(v) = val {
            f(self, v);
            self.inner.push(FIELD_SEP);
        }
        self
    }

    /// Some 时写入子项，None 时跳过
    #[inline]
    pub fn item_opt<T>(&mut self, val: Option<T>, f: impl FnOnce(&mut Self, T)) -> &mut Self {
        if let Some(v) = val {
            f(self, v);
            self.inner.push(ITEM_SEP);
        }
        self
    }

    /// Option + 映射：先 map 再决定是否写入字段
    /// ```ignore
    /// buf.field_opt_map(some_u32, |b, v| b.put_str(&v), |n| n.to_string());
    /// ```
    #[inline]
    pub fn field_opt_map<T, U>(
        &mut self,
        val: Option<T>,
        f: impl FnOnce(&mut Self, U),
        map: impl FnOnce(T) -> U,
    ) -> &mut Self {
        if let Some(v) = val {
            f(self, map(v));
            self.inner.push(FIELD_SEP);
        }
        self
    }

    // ---- 迭代器 ----

    /// 对每个元素写一个 item，整体作为一个 field
    /// ```ignore
    /// buf.field_iter(tool_calls.iter(), |b, call| {
    ///     b.put_str(&call.id);
    ///     b.put_str(&call.function_name);
    /// });
    /// // 产出: item0 0xFE item1 0xFE 0xFF
    /// ```
    #[inline]
    pub fn field_iter<T>(
        &mut self,
        iter: impl IntoIterator<Item = T>,
        f: impl Fn(&mut Self, T),
    ) -> &mut Self {
        for item in iter {
            f(self, item);
            self.inner.push(ITEM_SEP);
        }
        self.inner.push(FIELD_SEP);
        self
    }

    /// 条件为 true 时才迭代
    #[inline]
    pub fn field_iter_if<T>(
        &mut self,
        condition: bool,
        iter: impl IntoIterator<Item = T>,
        f: impl Fn(&mut Self, T),
    ) -> &mut Self {
        if condition {
            self.field_iter(iter, f);
        }
        self
    }

    // ---- 通用条件 ----

    /// 条件为 true 时执行任意操作
    #[inline]
    pub fn when(&mut self, condition: bool, f: impl FnOnce(&mut Self)) -> &mut Self {
        if condition {
            f(self);
        }
        self
    }

    /// match/if-else 的函数式替代
    /// ```ignore
    /// buf.field(|b| {
    ///     b.map(&content, |b, c| match c {
    ///         Content::Text(t) => { b.put_tag(b'T').put_str(t); }
    ///         Content::Tool(t) => { b.put_tag(b'C').put_str(&t.id); }
    ///     });
    /// });
    /// ```
    #[inline]
    pub fn map<T>(&mut self, val: &T, f: impl FnOnce(&mut Self, &T)) -> &mut Self {
        f(self, val);
        self
    }
}

// ===== Trait =====

pub trait SerializeMessage {
    fn serialize_message(&self, buf: &mut Buf);
}

// ===== HashChain =====

#[derive(Debug, Clone)]
pub struct HashChain {
    hashes: Vec<u128>,
}

impl HashChain {
    pub fn compute(messages: &[impl SerializeMessage]) -> Self {
        let mut hashes = Vec::with_capacity(messages.len());
        let mut buf = Buf::new();

        for msg in messages {
            buf.clear();

            if let Some(&prev) = hashes.last() {
                buf.put_u128_le(prev);
            }

            msg.serialize_message(&mut buf);
            hashes.push(xxh3_128(buf.as_bytes()));
        }

        Self { hashes }
    }

    pub fn current(&self) -> Option<u128> { self.hashes.last().copied() }

    pub fn prefix(&self) -> Option<u128> {
        (self.hashes.len() >= 2).then(|| self.hashes[self.hashes.len() - 2])
    }

    pub fn extend(&self, msg: &impl SerializeMessage) -> Self {
        let mut buf = Buf::new();

        if let Some(&last) = self.hashes.last() {
            buf.put_u128_le(last);
        }

        msg.serialize_message(&mut buf);

        let mut hashes = self.hashes.clone();
        hashes.push(xxh3_128(buf.as_bytes()));
        Self { hashes }
    }
}
