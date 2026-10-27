import * as Sentry from "@sentry/node";

const dsn = process.env.SENTRY_DSN;
if (!dsn) {
  console.error("Set SENTRY_DSN to the project DSN from the Statixo UI.");
  process.exit(1);
}

Sentry.init({
  dsn,
  environment: "production",
  release: "js-node@1.0.0",
  attachStacktrace: true,
});

Sentry.setUser({ id: "42", email: "ada@example.com", username: "ada" });
Sentry.setTag("example", "node");
Sentry.setTag("service", "checkout");

Sentry.addBreadcrumb({
  category: "example",
  message: "capturing a handled TypeError",
  level: "info",
});

try {
  throw new TypeError("Handled TypeError from the Node example");
} catch (error) {
  Sentry.captureException(error);
}

await Sentry.flush(5000);

setTimeout(() => {
  throw new Error("Uncaught error from the Node example");
}, 50);
