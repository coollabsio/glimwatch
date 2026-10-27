# Rust example

Uses the official [`sentry`](https://docs.sentry.io/platforms/rust/) crate.

```bash
cd examples/rust
SENTRY_DSN='http://<public_key>@127.0.0.1:43123/<project_id>' cargo run
```

The program captures `ExampleError` and a panic (caught so the process can flush). Both should appear in Statixo with stack traces.
