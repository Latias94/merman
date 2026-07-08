#!/usr/bin/env node
import { assertGeneratedSvgSafetyPolicyCurrent } from "./svg-safety-policy.mjs";

try {
  await assertGeneratedSvgSafetyPolicyCurrent();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
