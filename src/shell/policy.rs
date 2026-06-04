use serde_json::{Value, json};

pub const NATIVE_SHELL_TOOL_NAMES: &[&str] = &[
    "Bash",
    "bash",
    "Shell",
    "shell",
    "runTerminalCommand",
    "runInTerminal",
    "run_in_terminal",
    "terminal",
    "shell_command",
    "exec_command",
    "local_shell",
    "run_shell_command",
];

const FOLLOW_FLAGS: &[&str] = &["-f", "--follow"];
const ALWAYS_KEEP_NATIVE_PROGRAMS: &[&str] = &[
    "ssh", "scp", "sftp", "mosh", "top", "htop", "less", "more", "man", "vim", "nvim", "nano",
    "tmux", "screen", "watch",
];
const DOCKER_KEEP_NATIVE_SUBCOMMANDS: &[&str] = &["run", "exec", "attach"];
const DOCKER_COMPOSE_KEEP_NATIVE_SUBCOMMANDS: &[&str] = &["up", "exec", "run", "attach", "watch"];
const KUBECTL_KEEP_NATIVE_SUBCOMMANDS: &[&str] = &["exec", "attach", "port-forward"];
const CARGO_KEEP_NATIVE_SUBCOMMANDS: &[&str] = &["run", "watch"];
const JS_RUNNER_KEEP_NATIVE_SUBCOMMANDS: &[&str] = &["dev", "start", "serve", "watch", "create"];
const JS_RUNNER_KEEP_NATIVE_RUN_SUBCOMMANDS: &[&str] = &["dev", "start", "serve", "watch"];
const PYTHON_INTERACTIVE_FLAGS: &[&str] = &["-i"];
const PYTHON_KEEP_NATIVE_MODULES: &[&str] = &["http.server"];
const NODE_KEEP_NATIVE_FLAGS: &[&str] = &["--watch"];

pub fn should_prefer_so_shell(argv: &[String]) -> bool {
    let program_index = program_index(argv);

    let Some(program) = argv.get(program_index).map(|s| s.as_str()) else {
        return false;
    };
    let args = &argv[program_index + 1..];

    !should_keep_native_shell(base_program_name(program), args)
}

pub fn native_shell_policy_json() -> Value {
    json!({
        "shell_tool_names": NATIVE_SHELL_TOOL_NAMES,
        "follow_flags": FOLLOW_FLAGS,
        "always_keep_native_programs": ALWAYS_KEEP_NATIVE_PROGRAMS,
        "docker_keep_native_subcommands": DOCKER_KEEP_NATIVE_SUBCOMMANDS,
        "docker_compose_keep_native_subcommands": DOCKER_COMPOSE_KEEP_NATIVE_SUBCOMMANDS,
        "kubectl_keep_native_subcommands": KUBECTL_KEEP_NATIVE_SUBCOMMANDS,
        "cargo_keep_native_subcommands": CARGO_KEEP_NATIVE_SUBCOMMANDS,
        "js_runner_keep_native_subcommands": JS_RUNNER_KEEP_NATIVE_SUBCOMMANDS,
        "js_runner_keep_native_run_subcommands": JS_RUNNER_KEEP_NATIVE_RUN_SUBCOMMANDS,
        "python_interactive_flags": PYTHON_INTERACTIVE_FLAGS,
        "python_keep_native_modules": PYTHON_KEEP_NATIVE_MODULES,
        "python_keep_native_scripts": {
            "manage.py": ["runserver"],
        },
        "node_keep_native_flags": NODE_KEEP_NATIVE_FLAGS,
    })
}

fn should_keep_native_shell(program: &str, args: &[String]) -> bool {
    if ALWAYS_KEEP_NATIVE_PROGRAMS.contains(&program) {
        return true;
    }

    match program {
        "tail" => has_any_flag(args, FOLLOW_FLAGS),
        "docker" => !should_prefer_docker(args),
        "docker-compose" => !should_prefer_docker_compose(args),
        "kubectl" => !should_prefer_kubectl(args),
        "cargo" => {
            matches!(first_non_flag(args), Some(sub) if CARGO_KEEP_NATIVE_SUBCOMMANDS.contains(&sub))
        }
        "npm" | "pnpm" | "yarn" | "bun" | "npx" => should_keep_native_js_runner(args),
        "python" | "python3" => should_keep_native_python(args),
        "node" => has_any_flag(args, NODE_KEEP_NATIVE_FLAGS),
        _ => false,
    }
}

fn should_prefer_docker(args: &[String]) -> bool {
    let Some(subcommand) = first_non_flag(args) else {
        return true;
    };

    match subcommand {
        "logs" => !has_any_flag(args, FOLLOW_FLAGS),
        "compose" => should_prefer_docker_compose(after_subcommand(args, "compose")),
        _ => !DOCKER_KEEP_NATIVE_SUBCOMMANDS.contains(&subcommand),
    }
}

fn should_prefer_docker_compose(args: &[String]) -> bool {
    let Some(subcommand) = first_non_flag(args) else {
        return true;
    };

    match subcommand {
        "logs" => !has_any_flag(args, FOLLOW_FLAGS),
        _ => !DOCKER_COMPOSE_KEEP_NATIVE_SUBCOMMANDS.contains(&subcommand),
    }
}

fn should_prefer_kubectl(args: &[String]) -> bool {
    let Some(subcommand) = first_non_flag(args) else {
        return true;
    };

    match subcommand {
        "logs" => !has_any_flag(args, FOLLOW_FLAGS),
        _ => !KUBECTL_KEEP_NATIVE_SUBCOMMANDS.contains(&subcommand),
    }
}

fn should_keep_native_js_runner(args: &[String]) -> bool {
    let Some(subcommand) = first_non_flag(args) else {
        return false;
    };

    match subcommand {
        "run" => matches!(
            first_non_flag(after_subcommand(args, "run")),
            Some(nested) if JS_RUNNER_KEEP_NATIVE_RUN_SUBCOMMANDS.contains(&nested)
        ),
        _ => JS_RUNNER_KEEP_NATIVE_SUBCOMMANDS.contains(&subcommand),
    }
}

fn should_keep_native_python(args: &[String]) -> bool {
    if has_any_flag(args, PYTHON_INTERACTIVE_FLAGS) {
        return true;
    }

    match args.first().map(|arg| arg.as_str()) {
        Some("-m") => matches!(
            args.get(1).map(|arg| arg.as_str()),
            Some(module) if PYTHON_KEEP_NATIVE_MODULES.contains(&module)
        ),
        Some(script) if script.ends_with("manage.py") => {
            matches!(args.get(1).map(|arg| arg.as_str()), Some("runserver"))
        }
        _ => false,
    }
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

fn program_index(argv: &[String]) -> usize {
    if argv.first().map(|s| s.as_str()) == Some("env") {
        argv.iter()
            .skip(1)
            .take_while(|arg| is_env_assignment(arg))
            .count()
            + 1
    } else {
        0
    }
}

fn base_program_name(program: &str) -> &str {
    std::path::Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
}

fn has_any_flag(args: &[String], flags: &[&str]) -> bool {
    args.iter().any(|arg| flags.iter().any(|flag| arg == flag))
}

fn first_non_flag(args: &[String]) -> Option<&str> {
    args.iter()
        .find(|arg| !arg.starts_with('-'))
        .map(|s| s.as_str())
}

fn after_subcommand<'a>(args: &'a [String], subcommand: &str) -> &'a [String] {
    if let Some(index) = args.iter().position(|arg| arg == subcommand) {
        &args[index + 1..]
    } else {
        &[]
    }
}
