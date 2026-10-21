import * as Sentry from "@sentry/browser";

const dsn = window.BEACON_DSN;
Sentry.init({
  dsn,
  environment: "production",
  release: "js-browser@1.0.0",
});

Sentry.setUser({ id: "42", email: "ada@example.com", username: "ada" });
Sentry.setTag("example", "browser");
Sentry.setTag("service", "checkout");

document.getElementById("capture").addEventListener("click", () => {
  Sentry.addBreadcrumb({ category: "ui", message: "clicked manual capture", level: "info" });
  Sentry.captureException(new Error("Manual browser capture from the JavaScript example"));
  document.getElementById("status").textContent = "Sent a manual error.";
});

document.getElementById("throw").addEventListener("click", () => {
  Sentry.addBreadcrumb({ category: "ui", message: "clicked throw", level: "info" });
  throw new Error("Uncaught browser error from the JavaScript example");
});
