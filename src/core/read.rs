use std::{fs, path::PathBuf};

pub fn read(path: &str, mode: &str) -> Result<String, String> {
    let resolved = resolve_path(path).map_err(|e| format!("failed to resolve path: {e}"))?;
    let content =
        fs::read_to_string(&resolved).map_err(|e| format!("failed to read file: {e}"))?;

    match mode {
        "full" => Ok(content),
        "outline" => Ok(build_outline(&content)),
        other => Err(format!(
            "unsupported mode: {other}; expected full or outline"
        )),
    }
}

fn resolve_path(input: &str) -> std::io::Result<PathBuf> {
    let p = PathBuf::from(input);
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(std::env::current_dir()?.join(p))
    }
}

fn build_outline(content: &str) -> String {
    let mut out = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with('#')
            || t.starts_with("fn ")
            || t.starts_with("struct ")
            || t.starts_with("enum ")
            || t.starts_with("impl ")
            || t.starts_with("class ")
            || t.starts_with("interface ")
            || t.starts_with("def ")
        {
            out.push(format!("{}: {}", idx + 1, t));
        }
    }

    if out.is_empty() {
        "No outline items found.".to_string()
    } else {
        out.join("\n")
    }
}
