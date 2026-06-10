use super::*;

#[test]
fn keeps_compressed_mode_when_lengths_match() {
    let runner = ShellRunner::new(ShellRunOptions::new(false));
    let (rendered, mode) =
        runner.select_rendered_output("M  src/main.rs\n", "M  src/main.rs\n".to_string());

    assert_eq!(rendered, None);
    assert_eq!(mode, ShellOutputMode::RawFallback);
}

#[test]
fn falls_back_to_raw_when_compressed_output_is_longer() {
    let runner = ShellRunner::new(ShellRunOptions::new(false));
    let (rendered, mode) =
        runner.select_rendered_output("M  src/main.rs\n", "* main\nM  src/main.rs\n".to_string());

    assert_eq!(rendered, None);
    assert_eq!(mode, ShellOutputMode::RawFallback);
}
