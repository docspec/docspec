//! Child process for facade memory measurement.
//!
//! Drives [`AnyReader::from_path`] to completion on the DOCX given as the first
//! command-line argument, then prints its peak RSS to stderr.
//!
//! Usage: `memtest_facade_child <docx-path>`.

use docspec::{AnyReader, EventSource as _, InputFormat};

/// Reads this process's peak resident set size from `/proc/self/status`.
///
/// Every failure mode is propagated rather than defaulted to `0`: a zero
/// measurement would sit below the parent test's budget and turn the memory
/// regression test into a vacuous pass.
fn read_vm_hwm_kb() -> Result<u64, Box<dyn core::error::Error>> {
    let status = std::fs::read_to_string("/proc/self/status")?;
    let kilobytes = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .ok_or("VmHWM entry missing from /proc/self/status")?
        .split_whitespace()
        .next()
        .ok_or("VmHWM entry has no value")?;
    Ok(kilobytes.parse()?)
}

fn main() -> Result<(), Box<dyn core::error::Error>> {
    let path_arg = std::env::args()
        .nth(1)
        .ok_or("Usage: memtest_facade_child <docx-path>")?;

    let mut reader = AnyReader::from_path(InputFormat::Docx, &path_arg)?;
    while reader.next_event()?.is_some() {}

    let peak_kb = read_vm_hwm_kb()?;
    eprintln!("PEAK_RSS_KB={peak_kb}");
    Ok(())
}
