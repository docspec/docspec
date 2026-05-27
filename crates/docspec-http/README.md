# docspec-http

HTTP API server for [DocSpec](https://github.com/docspec/docspec) document conversion.

Exposes the DocSpec streaming pipeline over HTTP using [Axum 0.8](https://docs.rs/axum/0.8).
Accepts `text/markdown` input and returns `application/vnd.docspec.blocknote+json` output.

## Usage

Start the server programmatically:

```rust,no_run
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    docspec_http::serve("127.0.0.1", 3000).await
}
```

Or use the `docspec http` CLI subcommand:

```bash
docspec http --host 127.0.0.1 --port 3000
```

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/convert` | Convert Markdown to BlockNote JSON |
| `GET` | `/health` | Health check (204 No Content) |

## MIME Types

| Direction | MIME Type |
|-----------|-----------|
| Input | `text/markdown` |
| Output | `application/vnd.docspec.blocknote+json` |
| Errors | `application/problem+json` (RFC 7807) |

## Error Responses

All errors return [RFC 7807](https://www.rfc-editor.org/rfc/rfc7807) problem+json:

```json
{
  "type": "https://docspec.dev/errors/unsupported-media-type",
  "title": "Unsupported Media Type",
  "status": 415,
  "detail": "Content-Type 'application/json' is not supported. Use text/markdown."
}
```

## Documentation

See the [main DocSpec repository](https://github.com/docspec/docspec) for full documentation.

## License

MIT OR Apache-2.0
