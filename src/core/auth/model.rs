use super::error::AuthError;
use crate::app::model::ExtToken;

pub type TokenBundle = (ExtToken, bool);
pub type TokenBundleResult = Result<TokenBundle, AuthError>;
