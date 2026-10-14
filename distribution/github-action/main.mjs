import { setup } from './setup.mjs';

try {
  const installed = await setup({
    version: process.env.INPUT_VERSION,
    runnerTemp: process.env.RUNNER_TEMP,
    githubPath: process.env.GITHUB_PATH,
    githubOutput: process.env.GITHUB_OUTPUT,
  });
  console.log(`Installed merman-cli ${installed.version} at ${installed.path}`);
} catch (error) {
  console.error(`Setup Merman CLI failed: ${error.message}`);
  process.exitCode = 1;
}
