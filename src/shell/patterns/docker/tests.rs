use super::*;
use crate::shell::types::ShellInvocation;

#[test]
fn summarizes_ps_rows() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "ps".into()]),
        stdout: "NAMES IMAGE STATUS\nweb nginx Up\ndb postgres Up\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_ps(&result);
    assert_eq!(summary.summary, "2 containers; 2 running");
    assert_eq!(summary.details[0], "web — nginx (Up)");
}

#[test]
fn summarizes_inspect_json() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "inspect".into(), "web".into()]),
        stdout: "[{\"Id\":\"1234567890abcdef\",\"Name\":\"/web\",\"Image\":\"nginx:latest\",\"State\":{\"Status\":\"running\"}}]".into(),
        stderr: String::new(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_inspect(&result);
    assert_eq!(summary.summary, "1 inspect objects; name=web");
    assert_eq!(summary.details[0], "id=1234567890ab");
}

#[test]
fn summarizes_images_rows() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "images".into()]),
        stdout:
            "REPOSITORY  TAG  IMAGE ID  CREATED  SIZE\nnginx  latest  abc123  2 days ago  187MB\n"
                .into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_images(&result);
    assert_eq!(summary.summary, "1 images (187MB)");
    assert_eq!(summary.details[0], "nginx:latest — abc123 (187MB)");
}

#[test]
fn summarizes_images_with_kilobytes() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "images".into()]),
        stdout: "REPOSITORY  TAG  IMAGE ID  CREATED  SIZE\nfixture  latest  abc123  Less than a second ago  11.1kB\n".into(),
        stderr: String::new(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_images(&result);
    assert_eq!(summary.summary, "1 images (11.1kB)");
}

#[test]
fn summarizes_images_total_size_across_units() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "images".into()]),
        stdout: "REPOSITORY  TAG  IMAGE ID  CREATED  SIZE\nnginx  latest  abc123  2 days ago  512MB\npostgres  17  def456  3 days ago  1.5GB\n".into(),
        stderr: String::new(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_images(&result);
    assert_eq!(summary.summary, "2 images (2.0GB)");
}

#[test]
fn summarizes_compose_ps_table() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "compose".into(), "ps".into()]),
        stdout:
            "NAME  IMAGE  STATUS  PORTS\nweb-1  docker.io/library/node:20  running  0.0.0.0:3000->3000/tcp\n"
                .into(),
        stderr: String::new(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_compose(&result);
    assert_eq!(summary.summary, "1 services");
    assert_eq!(summary.details[0], "web-1 (node:20) running [3000]");
}

#[test]
fn summarizes_compose_ps_with_total_service_count() {
    let mut stdout = String::from("NAME  IMAGE  STATUS  PORTS\n");
    for index in 0..21 {
        stdout.push_str(&format!(
            "svc-{index}  docker.io/library/app:{index}  running  0.0.0.0:{}->3000/tcp\n",
            3000 + index
        ));
    }
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "compose".into(), "ps".into()]),
        stdout,
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_compose(&result);
    assert_eq!(summary.summary, "21 services");
    assert_eq!(summary.details.len(), 20);
}

#[test]
fn summarizes_pull_result() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "docker".into(),
            "pull".into(),
            "nginx:latest".into(),
        ]),
        stdout: "latest: Pulling from library/nginx\nDigest: sha256:abc\nStatus: Downloaded newer image for nginx:latest\n".into(),
        stderr: String::new(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_pull(&result);
    assert_eq!(
        summary.summary,
        "nginx:latest ok | Status: Downloaded newer image for nginx:latest"
    );
}

#[test]
fn summarizes_compose_fallback_output() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "compose".into(), "config".into()]),
        stdout: "services:\n  web:\n    image: nginx\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_compose(&result);
    assert_eq!(summary.summary, "compose output: 3 lines");
    assert_eq!(summary.details[0], "services:");
}

#[test]
fn summarizes_logs_prefers_error_lines() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "logs".into(), "web".into()]),
        stdout: "booting\nERROR failed to connect\nstill retrying\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_logs(&result);
    assert_eq!(summary.summary, "logs for web: 1 errors, 0 warnings");
    assert_eq!(summary.details[0], "ERROR failed to connect");
}

#[test]
fn summarizes_build_steps() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "build".into(), ".".into()]),
        stdout: "#1 [internal] load build definition from Dockerfile\n#2 [1/2] FROM rust:1.90\nSuccessfully tagged app:latest\n".into(),
        stderr: String::new(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_build(&result);
    assert_eq!(summary.summary, "Build successful");
    assert_eq!(
        summary.details[0],
        "#1 [internal] load build definition from Dockerfile"
    );
}

#[test]
fn summarizes_buildkit_output_without_digest_noise() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["docker".into(), "build".into(), ".".into()]),
        stdout: String::new(),
        stderr: "#0 building with \"desktop-linux\" instance using docker driver\n#1 [internal] load build definition from Dockerfile\n#1 transferring dockerfile: 179B done\n#1 DONE 0.0s\n#2 [internal] load .dockerignore\n#2 transferring context: 2B done\n#2 DONE 0.0s\n#3 [internal] load build context\n#3 transferring context: 79B done\n#3 DONE 0.0s\n#4 [1/1] COPY hello.txt /hello.txt\n#4 CACHED\n#5 exporting to image\n#5 exporting layers done\n#5 naming to moby-dangling@sha256:abc done\n#5 unpacking to moby-dangling@sha256:abc done\n#5 DONE 0.0s\n".into(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_build(&result);
    assert_eq!(summary.summary, "Build successful");
    assert!(summary.stderr_preview.is_empty());
    assert_eq!(summary.details[4], "#5 exporting to image");
}
