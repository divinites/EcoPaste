//! 按用户配置的正则排除剪贴板内容：文本按纯文本表示匹配，文件按换行连接的路径匹配。
//! 编译结果按规则列表缓存，规则不变时监听回调不重复编译。

use std::sync::Mutex;

use regex::{Regex, RegexBuilder};

use crate::core::{AppError, Result};
use crate::db::models::{ClipboardItem, ClipboardKind};
use crate::i18n::commands::Key;

/// 单条规则编译后的上限，防止病态规则占用过多内存。
const PATTERN_SIZE_LIMIT: usize = 1 << 20;

static CACHE: Mutex<Option<(Vec<String>, Vec<Regex>)>> = Mutex::new(None);

fn compile(pattern: &str) -> std::result::Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .size_limit(PATTERN_SIZE_LIMIT)
        .build()
}

/// 设置落盘前校验每条规则都能编译；错误带上出错的规则原文。
pub fn validate_excluded_patterns(patterns: &[String]) -> Result<()> {
    for pattern in patterns {
        if let Err(err) = compile(pattern) {
            log::warn!("invalid clipboard exclusion pattern {pattern:?}: {err}");
            return Err(AppError::localized_with(Key::FilterPatternInvalid, pattern));
        }
    }

    Ok(())
}

/// 判断记录是否命中任一排除规则；图片不参与匹配，无效规则跳过。
pub fn is_excluded_by_pattern(patterns: &[String], item: &ClipboardItem) -> bool {
    if patterns.is_empty() {
        return false;
    }

    let haystack = match item.kind {
        ClipboardKind::Text => item.search_text.as_deref().unwrap_or(&item.content),
        ClipboardKind::Files => item.content.as_str(),
        ClipboardKind::Image => return false,
    };

    let mut cache = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let stale = cache
        .as_ref()
        .is_none_or(|(cached, _)| cached.as_slice() != patterns);
    if stale {
        let compiled = patterns
            .iter()
            .filter_map(|pattern| compile(pattern).ok())
            .collect();
        *cache = Some((patterns.to_vec(), compiled));
    }

    cache
        .as_ref()
        .is_some_and(|(_, regexes)| regexes.iter().any(|regex| regex.is_match(haystack)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::{build_item, ClipboardPayload, ImageStore, TextPayload};

    fn text_item(text: &str) -> ClipboardItem {
        let store = ImageStore::for_test(std::env::temp_dir().join("ecopaste-filter-test"));
        build_item(
            &store,
            &ClipboardPayload::Text(TextPayload {
                text: text.to_owned(),
                html: None,
                rtf: None,
            }),
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn matches_text_and_ignores_empty_rules() {
        let item = text_item("Your verification code is 123456");

        assert!(!is_excluded_by_pattern(&[], &item));
        assert!(is_excluded_by_pattern(
            &[r"code is \d{6}".to_owned()],
            &item
        ));
        assert!(!is_excluded_by_pattern(&["^internal".to_owned()], &item));
    }

    #[test]
    fn invalid_rules_are_rejected_and_skipped() {
        let rules = vec!["(".to_owned(), "secret".to_owned()];

        assert!(validate_excluded_patterns(&rules).is_err());
        assert!(is_excluded_by_pattern(&rules, &text_item("top secret")));
    }
}
