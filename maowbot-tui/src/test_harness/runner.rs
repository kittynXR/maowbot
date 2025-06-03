use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::timeout;
use colored::Colorize;
use crate::test_harness::context::TestContext;

pub type TestFn = Box<dyn Fn(TestContext) -> Pin<Box<dyn Future<Output = TestResult> + Send>> + Send + Sync>;

#[derive(Debug)]
pub struct TestResult {
    pub passed: bool,
    pub message: String,
    pub assertions: Vec<Assertion>,
}

#[derive(Debug)]
pub struct Assertion {
    pub passed: bool,
    pub description: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

pub struct TestCase {
    pub name: String,
    pub description: Option<String>,
    pub test_fn: TestFn,
    pub timeout_seconds: u64,
}

pub struct TestRunner {
    tests: Vec<TestCase>,
}

impl TestRunner {
    pub fn new() -> Self {
        Self { tests: Vec::new() }
    }

    pub fn add_test<F, Fut>(mut self, name: impl Into<String>, test_fn: F) -> Self
    where
        F: Fn(TestContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = TestResult> + Send + 'static,
    {
        self.tests.push(TestCase {
            name: name.into(),
            description: None,
            test_fn: Box::new(move |ctx| Box::pin(test_fn(ctx))),
            timeout_seconds: 30,
        });
        self
    }

    pub fn add_test_with_description<F, Fut>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        test_fn: F,
    ) -> Self
    where
        F: Fn(TestContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = TestResult> + Send + 'static,
    {
        self.tests.push(TestCase {
            name: name.into(),
            description: Some(description.into()),
            test_fn: Box::new(move |ctx| Box::pin(test_fn(ctx))),
            timeout_seconds: 30,
        });
        self
    }

    pub async fn run(&self) -> TestSummary {
        let mut summary = TestSummary::default();
        
        println!("\n{}", "Running TUI Test Harness".bold().cyan());
        println!("{}", "=".repeat(50).cyan());

        for test in &self.tests {
            print!("Running test: {} ", test.name.bold());
            if let Some(desc) = &test.description {
                print!("({})", desc.dimmed());
            }
            println!();

            let ctx = TestContext::new();
            let start = std::time::Instant::now();

            let result = match timeout(
                Duration::from_secs(test.timeout_seconds),
                (test.test_fn)(ctx),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => TestResult {
                    passed: false,
                    message: format!("Test timed out after {} seconds", test.timeout_seconds),
                    assertions: vec![],
                },
            };

            let duration = start.elapsed();
            summary.total += 1;

            if result.passed {
                summary.passed += 1;
                println!("  {} ({:.2}s)", "✓ PASSED".green().bold(), duration.as_secs_f64());
            } else {
                summary.failed += 1;
                println!("  {} ({:.2}s)", "✗ FAILED".red().bold(), duration.as_secs_f64());
                println!("    {}: {}", "Message".yellow(), result.message);
            }

            // Print assertion details
            for assertion in &result.assertions {
                if assertion.passed {
                    println!("    {} {}", "✓".green(), assertion.description);
                } else {
                    println!("    {} {}", "✗".red(), assertion.description);
                    if let Some(expected) = &assertion.expected {
                        println!("      {}: {}", "Expected".yellow(), expected);
                    }
                    if let Some(actual) = &assertion.actual {
                        println!("      {}: {}", "Actual".yellow(), actual);
                    }
                }
            }

            println!();
        }

        summary.print();
        summary
    }
}

#[derive(Default)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

impl TestSummary {
    pub fn print(&self) {
        println!("{}", "=".repeat(50).cyan());
        println!("{}", "Test Summary".bold().cyan());
        println!(
            "Total: {} | {}: {} | {}: {}",
            self.total.to_string().bold(),
            "Passed".green().bold(),
            self.passed.to_string().green().bold(),
            "Failed".red().bold(),
            self.failed.to_string().red().bold()
        );

        if self.failed == 0 {
            println!("\n{}", "All tests passed! 🎉".green().bold());
        } else {
            println!("\n{}", "Some tests failed! 😞".red().bold());
        }
    }

    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

// Assertion builder
pub struct AssertionBuilder {
    assertions: Vec<Assertion>,
}

impl AssertionBuilder {
    pub fn new() -> Self {
        Self {
            assertions: Vec::new(),
        }
    }

    pub fn assert_eq<T: fmt::Debug + PartialEq>(
        mut self,
        description: impl Into<String>,
        expected: T,
        actual: T,
    ) -> Self {
        let passed = expected == actual;
        self.assertions.push(Assertion {
            passed,
            description: description.into(),
            expected: Some(format!("{:?}", expected)),
            actual: Some(format!("{:?}", actual)),
        });
        self
    }

    pub fn assert_true(mut self, description: impl Into<String>, value: bool) -> Self {
        self.assertions.push(Assertion {
            passed: value,
            description: description.into(),
            expected: Some("true".to_string()),
            actual: Some(value.to_string()),
        });
        self
    }

    pub fn assert_false(mut self, description: impl Into<String>, value: bool) -> Self {
        self.assertions.push(Assertion {
            passed: !value,
            description: description.into(),
            expected: Some("false".to_string()),
            actual: Some(value.to_string()),
        });
        self
    }

    pub fn assert_contains(
        mut self,
        description: impl Into<String>,
        haystack: &str,
        needle: &str,
    ) -> Self {
        let passed = haystack.contains(needle);
        self.assertions.push(Assertion {
            passed,
            description: description.into(),
            expected: Some(format!("contains '{}'", needle)),
            actual: Some(format!("'{}'", haystack)),
        });
        self
    }

    pub fn assert_not_contains(
        mut self,
        description: impl Into<String>,
        haystack: &str,
        needle: &str,
    ) -> Self {
        let passed = !haystack.contains(needle);
        self.assertions.push(Assertion {
            passed,
            description: description.into(),
            expected: Some(format!("does not contain '{}'", needle)),
            actual: Some(format!("'{}'", haystack)),
        });
        self
    }

    pub fn build(self) -> TestResult {
        let passed = self.assertions.iter().all(|a| a.passed);
        let message = if passed {
            "All assertions passed".to_string()
        } else {
            format!(
                "{} of {} assertions failed",
                self.assertions.iter().filter(|a| !a.passed).count(),
                self.assertions.len()
            )
        };

        TestResult {
            passed,
            message,
            assertions: self.assertions,
        }
    }
}

// Helper functions for common test patterns
pub fn success(message: impl Into<String>) -> TestResult {
    TestResult {
        passed: true,
        message: message.into(),
        assertions: vec![],
    }
}

pub fn failure(message: impl Into<String>) -> TestResult {
    TestResult {
        passed: false,
        message: message.into(),
        assertions: vec![],
    }
}

pub fn assert() -> AssertionBuilder {
    AssertionBuilder::new()
}