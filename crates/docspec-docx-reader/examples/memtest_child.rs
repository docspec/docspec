//! Child process for memory measurement.
//!
//! Reads the DOCX file at the path given as the first command-line argument,
//! drives DocxReader::from_path to completion, then prints the peak RSS to stderr.
//!
//! Usage: memtest_child <docx-path>
use std::path::Path;

use docspec_docx_reader::{DocxReader, EventSource};

fn read_vm_hwm_kb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in status.lines() {
        if line.starts_with("VmHWM:") {
            if let Some(kb) = line.split_whitespace().nth(1) {
                return kb.parse().unwrap_or(0);
            }
        }
    }
    0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path_arg = std::env::args()
        .nth(1)
        .ok_or("Usage: memtest_child <docx-path>")?;
    let path = Path::new(&path_arg);

    let mut reader = DocxReader::from_path(path)?;
    while reader.next_event()?.is_some() {}

    let peak_kb = read_vm_hwm_kb();
    eprintln!("PEAK_RSS_KB={peak_kb}");
    Ok(())
}
