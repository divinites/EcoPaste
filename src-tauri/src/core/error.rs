use std::fmt;

use serde::{ser::SerializeStruct, Serialize, Serializer};
use thiserror::Error;

use crate::i18n::commands::Key;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Clipboard(String),

    /// 直接展示给用户的根因：按当前界面语言渲染，`detail` 为可选的技术细节。
    #[error("{}", LocalizedMessage(*.0, .1.as_deref()))]
    Localized(Key, Option<String>),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AppError {
    /// 构造按当前界面语言渲染的用户可见错误。
    pub fn localized(key: Key) -> Self {
        Self::Localized(key, None)
    }

    /// 同 [`AppError::localized`]，附带一段技术细节（拼在文案之后）。
    pub fn localized_with(key: Key, detail: impl fmt::Display) -> Self {
        Self::Localized(key, Some(detail.to_string()))
    }

    fn kind(&self) -> &'static str {
        match self {
            AppError::Clipboard(_) | AppError::Localized(..) => "Clipboard",
            AppError::Other(_) => "Other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

struct LocalizedMessage<'a>(Key, Option<&'a str>);

impl fmt::Display for LocalizedMessage<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = crate::i18n::commands::label(crate::i18n::active_language(), self.0);
        match self.1 {
            Some(detail) => write!(f, "{label}: {detail}"),
            None => f.write_str(label),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Language;

    #[test]
    fn localized_error_follows_active_language() {
        crate::i18n::set_active_language(Language::EnUS);
        let err = AppError::localized_with(Key::SvgFileUnreadable, "denied");
        assert_eq!(err.to_string(), "Unable to read the SVG file: denied");

        crate::i18n::set_active_language(Language::ZhCN);
        assert_eq!(
            AppError::localized(Key::GroupNotFound).to_string(),
            "分组不存在"
        );
    }
}
