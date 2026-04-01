//! Serde 序列化支持（`feature = "serde"`）
//!
//! - **序列化**：输出字符串内容，与 `String` 的序列化格式完全一致。
//! - **反序列化**：先反序列化为 `String`，再通过池化创建 `ArcStr`。
//!   如果同一批数据中包含大量重复字符串，池化会自动去重。

use super::ArcStr;
use serde_core::{Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for ArcStr {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_str().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ArcStr {
    #[inline]
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(ArcStr::new)
    }
}
