const MAX_ACCOUNT_NAME_CHARS: usize = 32;

fn is_account_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || ('\u{4e00}'..='\u{9fff}').contains(&c)
}

/// 账号文件名白名单：1–32 个字符，仅字母、数字、下划线、中文和连字符。
pub fn validate_account_name(name: &str) -> Result<String, String> {
    if name.is_empty() || name.chars().count() > MAX_ACCOUNT_NAME_CHARS {
        return Err("账号名称长度需在1-32之间".to_string());
    }
    if !name.chars().all(is_account_id_char) {
        return Err("账号名称仅允许字母、数字、下划线、中文和连字符".to_string());
    }
    Ok(name.to_string())
}

/// 显示名与内部 id 分离：trim 后允许空格和 emoji，但拒绝空值、超长与控制字符。
pub fn validate_display_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let count = trimmed.chars().count();
    if count == 0 {
        return Err("显示名称不能为空".to_string());
    }
    if count > MAX_ACCOUNT_NAME_CHARS {
        return Err("显示名称长度需在1-32之间".to_string());
    }
    if trimmed.chars().any(char::is_control) {
        return Err("显示名称不得包含控制字符或换行".to_string());
    }
    Ok(trimmed.to_string())
}

/// 把白名单之外的字符替换为 `_`，按字符截断；与校验共用同一字符判据，避免双端漂移。
pub fn sanitize_account_id(user: &str) -> String {
    user.chars()
        .map(|c| if is_account_id_char(c) { c } else { '_' })
        .take(MAX_ACCOUNT_NAME_CHARS)
        .collect()
}
