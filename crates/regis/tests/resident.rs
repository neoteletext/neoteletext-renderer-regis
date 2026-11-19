//! The resident size a stream of numbered drawings makes a validator hold,
//! against the figure it reports and the bound (`SOLVEDSECURITYISSUES`
//! 015). One test alone in its process, so that nothing else allocates
//! while it measures; Linux only, where the process's resident size can be
//! read.

#![cfg(target_os = "linux")]

use std::fmt::Write as _;

use regis::Validator;
use regis::limits::MAX_HELD_BYTES;

/// What the resident size may grow by beyond the figure reported: the
/// pages the allocator rounds its heap to and the test's own few
/// allocations. On 19 November 2026 (`x86_64` Linux) the issue's stream grew
/// it by 2,461,696 bytes against 4,213,487 reported, so none was needed.
const SLACK_BYTES: usize = 512 * 1024;

/// The process's resident size, from `/proc/self/status`.
fn resident_bytes() -> usize {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let line = status.lines().find(|line| line.starts_with("VmRSS:")).unwrap();
    let kilobytes: usize = line.split_whitespace().nth(1).unwrap().parse().unwrap();
    kilobytes * 1024
}

#[test]
fn the_issue_stream_of_65535_empty_drawings_holds_no_more_than_reported_and_bounded() {
    // The issue's stream, built before measuring: 447,643 bytes.
    let mut stream = String::from("D(");
    for number in 1..=65_535 {
        write!(stream, "S{number},").unwrap();
    }
    stream.push_str("C)");
    assert_eq!(stream.len(), 447_643);
    let (open, commit) = stream.split_at(stream.len() - 2);

    let before = resident_bytes();
    let mut validator = Validator::new();
    // The staged scene at its fullest, just before the commit.
    let _ = validator.feed_regis(open.as_bytes());
    let grown = resident_bytes().saturating_sub(before);
    let held = validator.held_bytes();
    println!("resident size grew by {grown} bytes; the validator reports {held}; the bound is {MAX_HELD_BYTES}");
    assert!(grown <= MAX_HELD_BYTES, "grew by {grown}, past the bound {MAX_HELD_BYTES}");
    assert!(grown <= held + SLACK_BYTES, "grew by {grown}, more than the {held} reported");
    let _ = validator.feed_regis(commit.as_bytes());
    let grown = resident_bytes().saturating_sub(before);
    println!("after the commit: grew by {grown} bytes; the validator reports {}", validator.held_bytes());
    assert!(grown <= MAX_HELD_BYTES, "grew by {grown}, past the bound {MAX_HELD_BYTES}");
}
