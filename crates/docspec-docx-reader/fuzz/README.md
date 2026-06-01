# Fuzz Testing

Run the fuzz target with:

```bash
cargo +nightly fuzz run fuzz_docx_reader -p docspec-docx-reader -- -max_total_time=60
```
