import { defineConfig } from '@playwright/test';
import { existsSync } from 'node:fs';
const executablePath = process.env.WEBDECK_CHROMIUM ?? (existsSync('/usr/bin/chromium') ? '/usr/bin/chromium' : undefined);
export default defineConfig({
  testDir:'./e2e/acceptance', timeout:45000, workers:2, fullyParallel:true,
  use:{headless:true, trace:'retain-on-failure', launchOptions:{...(executablePath ? {executablePath} : {}), args:['--no-sandbox']}},
});
