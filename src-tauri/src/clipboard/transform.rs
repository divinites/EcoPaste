//! 文本记录的「粘贴为…」转换，以及多条记录合并为一段纯文本。
//! 转换只作用于写回剪贴板的副本，不修改历史记录本身。

use serde::{Deserialize, Serialize};

use crate::core::{AppError, Result};
use crate::db::models::{ClipboardItem, ClipboardKind};
use crate::i18n::commands::Key;

/// 右键菜单「粘贴为…」可选的转换，序列化为 camelCase 作为菜单项 / 命令参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextTransform {
    Trim,
    SingleLine,
    Uppercase,
    Lowercase,
    JsonPretty,
    JsonMinify,
    UrlEncode,
    UrlDecode,
}

impl TextTransform {
    /// 菜单展示顺序。
    pub const ALL: [TextTransform; 8] = [
        TextTransform::Trim,
        TextTransform::SingleLine,
        TextTransform::Uppercase,
        TextTransform::Lowercase,
        TextTransform::JsonPretty,
        TextTransform::JsonMinify,
        TextTransform::UrlEncode,
        TextTransform::UrlDecode,
    ];

    /// 与 serde 名一致的稳定 id，用于菜单项 id 与前端事件 payload。
    pub fn id(self) -> &'static str {
        match self {
            Self::Trim => "trim",
            Self::SingleLine => "singleLine",
            Self::Uppercase => "uppercase",
            Self::Lowercase => "lowercase",
            Self::JsonPretty => "jsonPretty",
            Self::JsonMinify => "jsonMinify",
            Self::UrlEncode => "urlEncode",
            Self::UrlDecode => "urlDecode",
        }
    }

    #[cfg(test)]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|transform| transform.id() == id)
    }

    /// 对纯文本执行转换；JSON / URL 解码失败返回用户可读错误。
    pub fn apply(self, text: &str) -> Result<String> {
        match self {
            Self::Trim => Ok(text.trim().to_owned()),
            Self::SingleLine => Ok(text.split_whitespace().collect::<Vec<_>>().join(" ")),
            Self::Uppercase => Ok(text.to_uppercase()),
            Self::Lowercase => Ok(text.to_lowercase()),
            Self::JsonPretty => {
                let value = parse_json(text)?;
                serde_json::to_string_pretty(&value).map_err(|err| AppError::Other(err.into()))
            }
            Self::JsonMinify => {
                let value = parse_json(text)?;
                serde_json::to_string(&value).map_err(|err| AppError::Other(err.into()))
            }
            Self::UrlEncode => Ok(url_encode(text)),
            Self::UrlDecode => url_decode(text),
        }
    }
}

fn parse_json(text: &str) -> Result<serde_json::Value> {
    serde_json::from_str(text.trim())
        .map_err(|err| AppError::localized_with(Key::TransformInvalidJson, err))
}

/// RFC 3986 百分号编码：保留非保留字符，其余按 UTF-8 字节编码。
fn url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// 百分号解码，`+` 视为空格（兼容表单编码）；结果必须是合法 UTF-8。
fn url_decode(text: &str) -> Result<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
                match hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                    Some(value) => {
                        out.push(value);
                        index += 3;
                    }
                    None => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(out).map_err(|_| AppError::localized(Key::TransformInvalidUrlEncoding))
}

/// 记录的可编辑 / 可转换纯文本：富文本用 OS 提供的纯文本表示，文件用换行分隔的路径。
/// 图片没有文本表示，返回 `None`。
pub fn plain_text_of(item: &ClipboardItem) -> Option<String> {
    match item.kind {
        ClipboardKind::Text => Some(
            item.search_text
                .clone()
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| item.content.clone()),
        ),
        ClipboardKind::Files => Some(item.content.clone()),
        ClipboardKind::Image => None,
    }
}

/// 把多条记录按给定顺序合并成一段纯文本，用换行分隔；图片被跳过。
pub fn merge_plain_text(items: &[ClipboardItem]) -> Option<String> {
    let parts: Vec<String> = items.iter().filter_map(plain_text_of).collect();
    (!parts.is_empty()).then(|| parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_text_transforms() {
        assert_eq!(TextTransform::Trim.apply("  a b \n").unwrap(), "a b");
        assert_eq!(
            TextTransform::SingleLine.apply(" a\n  b\tc ").unwrap(),
            "a b c"
        );
        assert_eq!(TextTransform::Uppercase.apply("Straße").unwrap(), "STRASSE");
        assert_eq!(TextTransform::Lowercase.apply("ABC").unwrap(), "abc");
    }

    #[test]
    fn json_transforms_validate_input() {
        assert_eq!(
            TextTransform::JsonMinify
                .apply("{ \"a\": [1, 2] }")
                .unwrap(),
            "{\"a\":[1,2]}"
        );
        assert_eq!(
            TextTransform::JsonPretty.apply("{\"a\":1}").unwrap(),
            "{\n  \"a\": 1\n}"
        );
        assert!(TextTransform::JsonPretty.apply("not json").is_err());
    }

    #[test]
    fn url_round_trip() {
        let encoded = TextTransform::UrlEncode.apply("a b/中").unwrap();
        assert_eq!(encoded, "a%20b%2F%E4%B8%AD");
        assert_eq!(TextTransform::UrlDecode.apply(&encoded).unwrap(), "a b/中");
        assert_eq!(
            TextTransform::UrlDecode.apply("a+b%zz%").unwrap(),
            "a b%zz%"
        );
        assert!(TextTransform::UrlDecode.apply("%FF").is_err());
    }

    #[test]
    fn ids_round_trip() {
        for transform in TextTransform::ALL {
            assert_eq!(TextTransform::from_id(transform.id()), Some(transform));
        }
    }
}
