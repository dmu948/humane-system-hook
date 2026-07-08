#!/usr/bin/env node
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function read(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function assertContains(text, needle, message) {
  if (!text.includes(needle)) {
    throw new Error(message);
  }
}

function assertMatches(text, pattern, message) {
  if (!pattern.test(text)) {
    throw new Error(message);
  }
}

const manifest = read("server/src/main/AndroidManifest.xml");
const bootReceiver = read("server/src/main/kotlin/com/penumbraos/server/BootReceiver.kt");
const docs = read("docs/AiPinServerDeployment.md");

assertContains(
  manifest,
  'android:sharedUserId="android.uid.system"',
  "server release manifest must remain a system-UID APK",
);
assertContains(
  manifest,
  '<action android:name="android.intent.action.BOOT_COMPLETED" />',
  "BootReceiver must handle BOOT_COMPLETED",
);
assertContains(
  manifest,
  '<action android:name="com.penumbraos.server.action.START_SERVER" />',
  "BootReceiver must expose the manual START_SERVER action",
);
assertMatches(
  manifest,
  /<service\s+[^>]*android:name="\.ServerService"[\s\S]*?android:exported="false"[\s\S]*?\/>/m,
  "ServerService must remain non-exported",
);
assertContains(
  bootReceiver,
  'ACTION_START_SERVER = "com.penumbraos.server.action.START_SERVER"',
  "BootReceiver must define ACTION_START_SERVER",
);
assertContains(
  docs,
  "Do not install `server-release.apk` with plain `pm install -r -d`",
  "deployment docs must warn against normal pm install",
);
assertContains(
  docs,
  "content://com.penumbraos.systeminjector.staging",
  "deployment docs must describe the system-injector staging provider",
);

console.log("server install contract checks passed");
