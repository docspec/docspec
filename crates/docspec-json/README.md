# docspec-json

JSON writing primitives for docspec writers.

This crate provides the JSON emission backend and emitter scaffolding for DocSpec writers.

## Streaming String Values

`JsonBackend::write_string_streaming` and `JsonEmitter::value_streaming` / `KeyedEmitter::value_streaming` allow writing a JSON string value whose content is produced incrementally:

```rust
emitter.key("data").value_streaming(|w| {
    write!(w, "prefix:")?;
    // write more bytes...
    Ok(())
})?;
```

**Always-close-string contract**: the JSON string is closed before the method returns, even if the callback returns `Err`. The produced JSON is structurally valid (matching quotes) but semantically incomplete (truncated content). Callers must discard partial output on error.
