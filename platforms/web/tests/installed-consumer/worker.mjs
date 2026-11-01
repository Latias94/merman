import { exercise } from "./exercise.mjs";

const cspViolations = [];
addEventListener("securitypolicyviolation", event => {
  cspViolations.push(`${event.effectiveDirective}: ${event.blockedURI}`);
});

void exercise("worker").then(
  result => postMessage({ result, cspViolations }),
  error => postMessage({ error: error.stack ?? String(error) }),
);
