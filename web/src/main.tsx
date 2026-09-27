// Self-hosted, subset to latin: no third-party request from a self-hosted
// application, and nothing to configuré on the VPS — Vite fingerprints the
// woff2 into the bundle the web image already serves.
//
// Deliberately NOT Inter: `layout/src/resolve.ts` and the seed brand name Inter
// for the collectives' own typography, and the renderer container only carries
// Liberation. Sharing the family would make the editor preview diverge from the
// server render — the one promise the Studio cannot break (§9).
import "@fontsource-variable/archivo";
import "@fontsource-variable/space-grotesk";
import "@fontsource-variable/jetbrains-mono";

import React from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import "./index.css";
import { applyStoredTheme } from "./lib/theme";

applyStoredTheme();

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>,
);
