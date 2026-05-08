/* @refresh reload */
import "@fontsource/geist/400.css";
import "@fontsource/geist/500.css";
import "@fontsource/geist/600.css";
import "@fontsource/geist-mono/400.css";
import "@fontsource/geist-mono/500.css";
import { render } from "solid-js/web";

import App from "./App";
import "./index.css";
import { installDebugSurface } from "./lib/debug";

// Install __pixhaus_debug__ + IPC tap before mount so e2e specs can read
// initial state and observe startup IPCs (e.g. crash_reporting_get_enabled).
// Wrapped defensively: a thrown exception here would prevent the app from
// mounting, leaving the user with a black screen.
try {
  installDebugSurface();
} catch (err) {
  console.warn("[pixhaus] failed to install debug surface:", err);
}

const root = document.getElementById("root");
if (!root) {
  throw new Error("missing #root mount node");
}

render(() => <App />, root);
