# PHP example

Uses the official [`sentry/sentry`](https://github.com/getsentry/sentry-php) package.

```bash
cd examples/php
composer install
SENTRY_DSN='http://<public_key>@127.0.0.1:43123/<project_id>' php index.php
```

The script captures one `RuntimeException` manually, then throws an uncaught `InvalidArgumentException`. Both should show up as issues in Owlee, with stack frames and context lines.
