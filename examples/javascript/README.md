# JavaScript examples

Uses the official [`@sentry/node`](https://docs.sentry.io/platforms/javascript/guides/node/) and [`@sentry/browser`](https://docs.sentry.io/platforms/javascript/) SDKs.

```bash
cd examples/javascript
npm install
SENTRY_DSN='http://<public_key>@127.0.0.1:43123/<project_id>' npm run node
```

The Node script captures a handled `TypeError`, flushes it, then throws an uncaught `Error`.

Browser page:

```bash
SENTRY_DSN='http://<public_key>@127.0.0.1:43123/<project_id>' npm run browser
```

Open the printed URL and click **Capture error** or **Throw error**.
