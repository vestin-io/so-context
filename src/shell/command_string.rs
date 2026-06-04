pub fn parse_simple_shell_command(command: &str) -> Option<Vec<String>> {
    if command.contains('\n') || command.contains('\r') {
        return None;
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            } else if ch == '\\' && active == '"' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            } else {
                current.push(ch);
            }
            continue;
        }

        match ch {
            '\'' | '"' => quote = Some(ch),
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ' ' | '\t' => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            '|' | '&' | ';' | '<' | '>' | '`' => return None,
            '$' => return None,
            '(' | ')' => return None,
            _ => current.push(ch),
        }
    }

    if quote.is_some() {
        return None;
    }
    if !current.is_empty() {
        args.push(current);
    }
    if args.is_empty() {
        return None;
    }

    Some(args)
}

pub fn rewrite_env_prefix(argv: Vec<String>) -> Vec<String> {
    let env_prefix_len = argv.iter().take_while(|arg| is_env_assignment(arg)).count();

    if env_prefix_len == 0 {
        return argv;
    }

    let mut rewritten = Vec::with_capacity(argv.len() + 1);
    rewritten.push("env".to_string());
    rewritten.extend(argv);
    rewritten
}

fn is_env_assignment(arg: &str) -> bool {
    let Some((name, _value)) = arg.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
