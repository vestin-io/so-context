use super::super::super::types::ShellPattern;
use super::super::argv::{first_positional, positional_args};

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    match program {
        "vitest" => Some(ShellPattern::Vitest),
        "jest" => Some(ShellPattern::Jest),
        "playwright" if first_positional(args, &[]).is_some_and(|arg| arg == "test") => {
            Some(ShellPattern::PlaywrightTest)
        }
        "go" if first_positional(args, &[]).is_some_and(|arg| arg == "test") => {
            Some(ShellPattern::GoTest)
        }
        "rspec" => Some(ShellPattern::Rspec),
        "minitest" => Some(ShellPattern::Minitest),
        "bundle" => classify_bundle(args),
        "ruby" => classify_ruby(args),
        "rails" | "./bin/rails" | "bin/rails"
            if first_positional(args, &[]).is_some_and(|arg| arg == "test") =>
        {
            Some(ShellPattern::Minitest)
        }
        "npx" | "bunx" => classify_wrapped_positionals(&positionals(args, &[])),
        "bun" => classify_bun(args),
        "npm" => classify_npm(args),
        "pnpm" => classify_pnpm(args),
        "yarn" => classify_yarn(args),
        _ => None,
    }
}

fn classify_bundle(args: &[String]) -> Option<ShellPattern> {
    if args.first().map(String::as_str) != Some("exec") {
        return None;
    }

    match args.get(1).map(String::as_str) {
        Some("rspec") => Some(ShellPattern::Rspec),
        Some("ruby") => classify_ruby(&args[2..]),
        Some("rails") | Some("./bin/rails") | Some("bin/rails")
            if args.get(2).map(String::as_str) == Some("test") =>
        {
            Some(ShellPattern::Minitest)
        }
        _ => None,
    }
}

fn classify_ruby(args: &[String]) -> Option<ShellPattern> {
    if args.iter().any(|arg| arg == "rspec") {
        return Some(ShellPattern::Rspec);
    }

    if args.iter().any(|arg| arg == "-Itest" || arg == "test")
        || args.iter().any(|arg| arg.ends_with("_test.rb"))
    {
        return Some(ShellPattern::Minitest);
    }

    None
}

fn classify_wrapped_positionals(positionals: &[&str]) -> Option<ShellPattern> {
    let tool = *positionals.first()?;
    let trailing = &positionals[1..];

    match tool {
        "vitest" => Some(ShellPattern::Vitest),
        "jest" => Some(ShellPattern::Jest),
        "playwright" if trailing.first().copied() == Some("test") => {
            Some(ShellPattern::PlaywrightTest)
        }
        "rspec" => Some(ShellPattern::Rspec),
        "minitest" => Some(ShellPattern::Minitest),
        _ => None,
    }
}

fn classify_bun(args: &[String]) -> Option<ShellPattern> {
    let positionals = positionals(args, &["--cwd"]);
    match positionals.first().copied() {
        Some("x") | Some("run") => classify_wrapped_positionals(&positionals[1..]),
        _ => classify_wrapped_positionals(&positionals),
    }
}

fn classify_npm(args: &[String]) -> Option<ShellPattern> {
    let positionals = positionals(args, &["--prefix", "--cache", "-w", "--workspace"]);
    match positionals.first().copied() {
        Some("exec") | Some("x") | Some("run") | Some("run-script") => {
            classify_wrapped_positionals(&positionals[1..])
        }
        _ => classify_wrapped_positionals(&positionals),
    }
}

fn classify_pnpm(args: &[String]) -> Option<ShellPattern> {
    let positionals = positionals(args, &["--filter", "-C", "--dir"]);
    match positionals.first().copied() {
        Some("exec") | Some("dlx") | Some("run") | Some("run-script") => {
            classify_wrapped_positionals(&positionals[1..])
        }
        _ => classify_wrapped_positionals(&positionals),
    }
}

fn classify_yarn(args: &[String]) -> Option<ShellPattern> {
    let positionals = positionals(args, &["--cwd"]);
    match positionals.first().copied() {
        Some("dlx") | Some("run") => classify_wrapped_positionals(&positionals[1..]),
        _ => classify_wrapped_positionals(&positionals),
    }
}

fn positionals<'a>(args: &'a [String], flags_with_values: &[&str]) -> Vec<&'a str> {
    positional_args(args, flags_with_values)
}
