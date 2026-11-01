//! Parses plain-text command suggestions.

const PREDICT_MAX_LEN: usize = 200;

/// 解析 nld_predict 的纯文本输出。
/// trim,剥外引号,拒绝多行 / 超长 → `None`。
pub fn sanitize_predict(raw: &str) -> Option<String> {
    let mut s = raw.trim().to_owned();
    if s.is_empty() {
        return None;
    }
    if s.contains('\n') {
        // 取首行(模型有时多嘴解释)
        s = s.lines().next().unwrap_or("").trim().to_owned();
        if s.is_empty() {
            return None;
        }
    }
    let quotes = ['"', '\'', '`', '“', '”', '‘', '’'];
    if let Some(c) = s.chars().next() {
        if quotes.contains(&c) {
            s.remove(0);
        }
    }
    if let Some(c) = s.chars().last() {
        if quotes.contains(&c) {
            let new_len = s.len() - c.len_utf8();
            s.truncate(new_len);
        }
    }
    let s = s.trim().to_owned();
    if s.is_empty() || s.chars().count() > PREDICT_MAX_LEN {
        return None;
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_basic() {
        assert_eq!(
            sanitize_predict("hello world").as_deref(),
            Some("hello world")
        );
    }

    #[test]
    fn sanitize_strip_quotes() {
        assert_eq!(sanitize_predict("\"foo\"").as_deref(), Some("foo"));
    }

    #[test]
    fn sanitize_multiline_takes_first() {
        assert_eq!(
            sanitize_predict("first line\nsecond").as_deref(),
            Some("first line")
        );
    }

    #[test]
    fn sanitize_empty_returns_none() {
        assert!(sanitize_predict("").is_none());
        assert!(sanitize_predict("   ").is_none());
    }
}
