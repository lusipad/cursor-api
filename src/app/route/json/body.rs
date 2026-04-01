use super::InfallibleJson;
use crate::{
    common::model::{ApiStatus, GenericError},
    core::{
        error::ErrorExt,
        model::{
            anthropic::{AnthropicError, AnthropicErrorInner},
            openai::{OpenAiError, OpenAiErrorInner},
        },
    },
};
use alloc::borrow::Cow;
use http::StatusCode;

#[cfg(feature = "__perf")]
use sonic_rs as serde_json;

#[derive(Debug)]
pub(super) enum JsonBodyError {
    /// JSON syntax error (malformed JSON, unexpected EOF)
    Syntax { message: String },
    /// JSON data error (type mismatch, missing field, etc.)
    Data { message: String },
}

impl JsonBodyError {
    fn from_serde(err: serde_path_to_error::Error<serde_json::Error>) -> Self {
        let message = err.to_string();
        match err.inner().classify() {
            #[cfg(not(feature = "__perf"))]
            serde_json::error::Category::Data => Self::Data { message },
            #[cfg(feature = "__perf")]
            serde_json::error::Category::TypeUnmatched | serde_json::error::Category::NotFound => {
                Self::Data { message }
            }
            serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
                Self::Syntax { message }
            }
            serde_json::error::Category::Io => {
                // SAFETY: we deserialize from &[u8] via from_slice,
                // IO errors are impossible without a Reader
                unsafe { core::hint::unreachable_unchecked() }
            }
            #[cfg(feature = "__perf")]
            _ => Self::Syntax { message },
        }
    }
}

impl JsonBodyError {
    #[inline]
    fn triple(self) -> (StatusCode, &'static str, Cow<'static, str>) {
        match self {
            Self::Syntax { message } => {
                (StatusCode::BAD_REQUEST, "json_syntax_error", Cow::Owned(message))
            }
            Self::Data { message } => {
                (StatusCode::UNPROCESSABLE_ENTITY, "json_data_error", Cow::Owned(message))
            }
        }
    }
}

impl ErrorExt for JsonBodyError {
    #[inline]
    fn into_generic_tuple(self) -> (StatusCode, InfallibleJson<GenericError>) {
        let (status, error_type, message) = self.triple();
        (
            status,
            InfallibleJson(GenericError {
                status: ApiStatus::Error,
                code: Some(status),
                error: Some(Cow::Borrowed(error_type)),
                message: Some(message),
            }),
        )
    }

    #[inline]
    fn into_openai_tuple(self) -> (StatusCode, InfallibleJson<OpenAiError>) {
        let (status, error_type, message) = self.triple();
        (
            status,
            InfallibleJson(
                OpenAiErrorInner { code: Some(Cow::Borrowed(error_type)), message }.wrapped(),
            ),
        )
    }

    #[inline]
    fn into_anthropic_tuple(self) -> (StatusCode, InfallibleJson<AnthropicError>) {
        let (status, error_type, message) = self.triple();
        (status, InfallibleJson(AnthropicErrorInner { r#type: error_type, message }.wrapped()))
    }
}

pub(super) fn from_bytes<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, JsonBodyError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);

    serde_path_to_error::deserialize(&mut deserializer).map_err(JsonBodyError::from_serde).and_then(
        |value| {
            deserializer
                .end()
                .map(|()| value)
                .map_err(|err| JsonBodyError::Syntax { message: err.to_string() })
        },
    )
}
