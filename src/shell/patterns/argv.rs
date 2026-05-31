pub(super) fn first_positional<'a>(
    args: &'a [String],
    flags_with_values: &[&str],
) -> Option<&'a str> {
    positional_args(args, flags_with_values).into_iter().next()
}

pub(super) fn positional_args<'a>(args: &'a [String], flags_with_values: &[&str]) -> Vec<&'a str> {
    let mut values = Vec::new();
    let mut skip_next = false;
    let mut passthrough = false;

    for arg in args {
        if passthrough {
            values.push(arg.as_str());
            continue;
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--" {
            passthrough = true;
            continue;
        }
        if flags_with_values.iter().any(|flag| arg == flag) {
            skip_next = true;
            continue;
        }
        if flags_with_values
            .iter()
            .filter(|flag| flag.starts_with("--"))
            .any(|flag| arg.starts_with(&format!("{flag}=")))
        {
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        values.push(arg.as_str());
    }

    values
}
