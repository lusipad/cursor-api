#[cfg(not(feature = "__perf"))]
use serde_json as sonic_rs;

pub struct ToolContentBuilder {
    content: Vec<RawContent>,
    is_error: bool,
}

#[derive(::serde::Serialize)]
struct RawTextContent {
    text: String,
}

#[derive(::serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RawImageContent {
    /// The base64-encoded image
    data: String,
    mime_type: &'static str,
}

#[allow(private_interfaces)]
#[derive(::serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RawContent {
    Text(RawTextContent),
    Image(RawImageContent),
}

impl From<String> for RawContent {
    #[inline]
    fn from(text: String) -> Self { Self::Text(RawTextContent { text }) }
}

impl From<(String, &'static str)> for RawContent {
    #[inline]
    fn from(args: (String, &'static str)) -> Self {
        let (data, mime_type) = args;
        Self::Image(RawImageContent { data, mime_type })
    }
}

impl RawContent {
    fn estimate_length(&self) -> usize {
        match self {
            RawContent::Text(RawTextContent { text }) => 25 + text.len(),
            RawContent::Image(RawImageContent { data, mime_type }) => {
                40 + data.len() + mime_type.len()
            }
        }
    }

    pub fn text(text: String) -> Self { Self::Text(RawTextContent { text }) }
    pub fn image(base64_data: String, mime_type: &'static str) -> Self {
        Self::Image(RawImageContent { data: base64_data, mime_type })
    }
}

impl ToolContentBuilder {
    fn estimate_length(&self) -> usize {
        let mut len = 11;

        let content_len: usize = self.content.iter().map(RawContent::estimate_length).sum();
        if self.is_error {
            len += 18;
            len += content_len;
            len += self.content.len().saturating_sub(1);
        } else {
            len += 53;
            len += content_len * 2;
            len += self.content.len() * 32;
            len += self.content.len().saturating_sub(1) * 2;
        }

        // 给转义字符预留12.5%
        len + (len >> 3)
    }

    pub fn build(self) -> String {
        const PAT: &[u8] = b",\"annotations\":null,\"_meta\":null}";
        let mut json = Vec::with_capacity(self.estimate_length());

        json.extend_from_slice(b"{\"content\":");
        if self.is_error {
            unsafe { sonic_rs::to_writer(&mut json, &self.content).unwrap_unchecked() };
            json.extend_from_slice(b",\"isError\":true}");
        } else {
            let mut indexes = Vec::with_capacity(self.content.len());
            json.push(b'[');
            if let Some(first) = self.content.first() {
                let start = json.len();
                unsafe { sonic_rs::to_writer(&mut json, first).unwrap_unchecked() };
                let end = json.len() - 1;
                indexes.push(start..end);
            }
            if self.content.len() > 1 {
                for after in &self.content[1..] {
                    let start = json.len();
                    json.push(b',');
                    unsafe { sonic_rs::to_writer(&mut json, after).unwrap_unchecked() };
                    let end = json.len() - 1;
                    indexes.push(start..end);
                }
            }
            json.extend_from_slice(b"],\"structuredContent\":{\"result\":[");
            for range in indexes {
                json.extend_from_within(range);
                json.extend_from_slice(PAT);
            }
            json.extend_from_slice(b"]},\"isError\":false}");
        }

        unsafe { String::from_utf8_unchecked(json) }
    }

    pub fn new(is_error: bool) -> Self { Self { content: Vec::new(), is_error } }

    pub fn with_capacity(capacity: usize, is_error: bool) -> Self {
        Self { content: Vec::with_capacity(capacity), is_error }
    }

    pub fn add(&mut self, content: impl Into<RawContent>) { self.content.push(content.into()) }
}
