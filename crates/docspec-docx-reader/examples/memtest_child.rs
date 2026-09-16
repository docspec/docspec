//! Child process for memory measurement.
//!
//! Reads the DOCX file path, drives either streaming constructor to completion,
//! then prints the peak RSS to stderr.
//!
//! Usage: `memtest_child [--reader-streaming] <docx-path>`.
use std::path::Path;

use docspec_docx_reader::{DocxReader, EventSource as _};

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

fn main() -> Result<(), Box<dyn core::error::Error>> {
    let mut args = std::env::args().skip(1);
    let first = args
        .next()
        .ok_or("Usage: memtest_child [--reader-streaming] <docx-path>")?;
    let (reader_streaming, path_arg) = if first == "--reader-streaming" {
        (true, args.next().ok_or("missing DOCX path")?)
    } else {
        (false, first)
    };
    let path = Path::new(&path_arg);

    let mut reader = if reader_streaming {
        DocxReader::from_reader_streaming(std::fs::File::open(path)?)?
    } else {
        DocxReader::from_path(path)?
    };
    while reader.next_event()?.is_some() {}

    let peak_kb = read_vm_hwm_kb();
    eprintln!("PEAK_RSS_KB={peak_kb}");
    Ok(())
}
