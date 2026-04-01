//! Serde 序列化支持（`feature = "serde"`）
//!
//! - **序列化**：输出字符串内容，不保留变体信息。
//! - **反序列化**：总是生成 `Counted` 变体——
//!   反序列化的数据不具备 `'static` 生命周期，无法恢复 `Static`。

use super::Str;
use serde_core::{Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for Str {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_str().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Str {
    #[inline]
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Str::from)
    }
}
