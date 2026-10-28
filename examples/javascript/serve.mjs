import * as esbuild from "esbuild";
import http from "node:http";
import { readFileSync } from "node:fs";

const dsn = process.env.SENTRY_DSN;
if (!dsn) {
  console.error("Set SENTRY_DSN to the project DSN from the Glimwatch UI.");
  process.exit(1);
}

const port = Number(process.env.PORT || 43130);
await esbuild.build({
  entryPoints: ["browser-entry.js"],
  bundle: true,
  format: "iife",
  outfile: "dist/browser.js",
  platform: "browser",
});

const page = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Glimwatch browser example</title>
</head>
<body>
  <h1>Glimwatch browser example</h1>
  <p>This page uses the official @sentry/browser SDK.</p>
  <button id="capture" type="button">Capture error</button>
  <button id="throw" type="button">Throw error</button>
  <p id="status"></p>
  <script>window.BEACON_DSN = ${JSON.stringify(dsn)};</script>
  <script src="/browser.js"></script>
</body>
</html>`;

const bundle = readFileSync(new URL("./dist/browser.js", import.meta.url));
const server = http.createServer((request, response) => {
  if (request.url === "/browser.js") {
    response.writeHead(200, { "content-type": "text/javascript" });
    response.end(bundle);
    return;
  }
  response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  response.end(page);
});

server.listen(port, "127.0.0.1", () => {
  console.log(`browser example at http://127.0.0.1:${port}`);
});
