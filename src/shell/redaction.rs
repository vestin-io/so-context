pub(super) fn redact_argv(argv: &[String]) -> Vec<String> {
    let mut redacted = Vec::with_capacity(argv.len());
    let mut redact_next = false;

    for arg in argv {
        if redact_next {
            redacted.push(redact_sensitive_value(arg));
            redact_next = false;
            continue;
        }

        if let Some((flag, value)) = arg.split_once('=')
            && is_sensitive_flag(flag)
        {
            redacted.push(format!("{flag}={}", redact_sensitive_value(value)));
            continue;
        }

        if is_sensitive_flag(arg) {
            redacted.push(arg.clone());
            redact_next = true;
            continue;
        }

        redacted.push(redact_inline_secret(arg));
    }

    redacted
}

fn redact_inline_secret(arg: &str) -> String {
    if arg.contains("Authorization:") || arg.contains("authorization:") || arg.contains("Bearer ") {
        return redact_sensitive_value(arg);
    }

    if let Some((prefix, query)) = arg.split_once('?') {
        let rewritten = query
            .split('&')
            .map(|pair| {
                let Some((key, value)) = pair.split_once('=') else {
                    return pair.to_string();
                };
                if is_sensitive_key(key) {
                    format!("{key}={}", redact_sensitive_value(value))
                } else {
                    format!("{key}={value}")
                }
            })
            .collect::<Vec<_>>()
            .join("&");
        return format!("{prefix}?{rewritten}");
    }

    arg.to_string()
}

fn redact_sensitive_value(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        "[REDACTED]".to_string()
    }
}

fn is_sensitive_flag(flag: &str) -> bool {
    matches!(
        flag,
        "-u" | "--user"
            | "-H"
            | "--header"
            | "--token"
            | "--password"
            | "--passwd"
            | "--secret"
            | "--api-key"
            | "--apikey"
            | "--auth"
            | "--authorization"
    )
}

fn is_sensitive_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    ["TOKEN", "SECRET", "PASSWORD", "KEY", "CREDENTIAL", "AUTH"]
        .iter()
        .any(|pattern| upper.contains(pattern))
}
