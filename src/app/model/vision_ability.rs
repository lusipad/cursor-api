use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum VisionAbility {
    None,
    Base64,
    All,
}

impl VisionAbility {
    /// 主要的字符串表示
    const NONE: &'static str = "none";
    const BASE64: &'static str = "base64";
    const ALL: &'static str = "all";

    /// 别名
    const NONE_ALIAS: &'static str = "disabled";
    const BASE64_ALIAS: &'static str = "base64-only";
    const ALL_ALIAS: &'static str = "base64-http";

    #[inline]
    fn from_string(mut s: String) -> Self {
        s.make_ascii_lowercase();
        match s.as_str() {
            Self::NONE | Self::NONE_ALIAS => Self::None,
            Self::BASE64 | Self::BASE64_ALIAS => Self::Base64,
            Self::ALL | Self::ALL_ALIAS => Self::All,
            _ => Self::default(),
        }
    }

    #[inline(always)]
    pub fn is_none(self) -> bool { matches!(self, VisionAbility::None) }

    /// 获取枚举的主要字符串表示
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => Self::NONE,
            Self::Base64 => Self::BASE64,
            Self::All => Self::ALL,
        }
    }
}

impl const Default for VisionAbility {
    #[inline(always)]
    fn default() -> Self { Self::Base64 }
}

impl Serialize for VisionAbility {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for VisionAbility {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_string(s))
    }
}
