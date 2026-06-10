use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct JsTestJsonOutput {
    #[serde(rename = "testResults", default)]
    pub(super) test_results: Vec<JsTestFile>,
    #[serde(rename = "numPassedTests", default)]
    pub(super) num_passed_tests: usize,
    #[serde(rename = "numFailedTests", default)]
    pub(super) num_failed_tests: usize,
    #[serde(rename = "numPendingTests", default)]
    pub(super) num_pending_tests: usize,
}

#[derive(Debug, Deserialize)]
pub(super) struct JsTestFile {
    #[serde(default)]
    pub(super) name: String,
    #[serde(rename = "assertionResults", default)]
    pub(super) assertion_results: Vec<JsAssertionResult>,
}

#[derive(Debug, Deserialize)]
pub(super) struct JsAssertionResult {
    #[serde(rename = "fullName", default)]
    pub(super) full_name: String,
    #[serde(default)]
    pub(super) status: String,
    #[serde(rename = "failureMessages", default)]
    pub(super) failure_messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PlaywrightJsonOutput {
    pub(super) stats: PlaywrightStats,
    #[serde(default)]
    pub(super) suites: Vec<PlaywrightSuite>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PlaywrightStats {
    #[serde(default)]
    pub(super) expected: usize,
    #[serde(default)]
    pub(super) unexpected: usize,
    #[serde(default)]
    pub(super) skipped: usize,
    #[serde(default)]
    pub(super) duration: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct PlaywrightSuite {
    pub(super) title: String,
    #[serde(default)]
    pub(super) file: Option<String>,
    #[serde(default)]
    pub(super) specs: Vec<PlaywrightSpec>,
    #[serde(default)]
    pub(super) suites: Vec<PlaywrightSuite>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PlaywrightSpec {
    pub(super) title: String,
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) tests: Vec<PlaywrightExecution>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PlaywrightExecution {
    pub(super) status: String,
    #[serde(default)]
    pub(super) results: Vec<PlaywrightAttempt>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PlaywrightAttempt {
    pub(super) status: String,
    #[serde(default)]
    pub(super) errors: Vec<PlaywrightError>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PlaywrightError {
    #[serde(default)]
    pub(super) message: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoTestEvent {
    #[serde(rename = "Action")]
    pub(super) action: String,
    #[serde(rename = "Package")]
    pub(super) package: Option<String>,
    #[serde(rename = "Test")]
    pub(super) test: Option<String>,
    #[serde(rename = "Output")]
    pub(super) output: Option<String>,
    #[serde(rename = "Elapsed")]
    pub(super) elapsed: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RspecOutput {
    pub(super) examples: Vec<RspecExample>,
    pub(super) summary: RspecSummary,
}

#[derive(Debug, Deserialize)]
pub(super) struct RspecExample {
    pub(super) full_description: String,
    pub(super) status: String,
    pub(super) file_path: String,
    pub(super) line_number: u32,
    pub(super) exception: Option<RspecException>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RspecException {
    pub(super) class: String,
    pub(super) message: String,
    #[serde(default)]
    pub(super) backtrace: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RspecSummary {
    pub(super) duration: f64,
    pub(super) example_count: usize,
    pub(super) failure_count: usize,
    pub(super) pending_count: usize,
    #[serde(default)]
    pub(super) errors_outside_of_examples_count: usize,
}
