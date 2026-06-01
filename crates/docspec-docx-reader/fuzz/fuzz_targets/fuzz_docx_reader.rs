#![no_main]
use docspec_docx_reader::EventSource as _;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let cursor = Cursor::new(data);
    if let Ok(mut reader) = docspec_docx_reader::DocxReader::new(cursor) {
        // Drain up to a bounded number of events so we don't loop on a giant valid file.
        for _ in 0..10_000 {
            match reader.next_event() {
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => break,
            }
        }
    }
});
