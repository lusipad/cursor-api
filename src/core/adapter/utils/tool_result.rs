use super::super::traits::ToolContent;
use byte_str::ByteStr;

pub struct ToolResult<C: ToolContent> {
    pub content: C,
    pub id: ByteStr,
    pub name: ByteStr,
}
