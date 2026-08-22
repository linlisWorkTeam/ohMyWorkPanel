import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { requiresAuth } from "./api";
import { ThemeProvider } from "./theme";
import "./shell/tokens.css";
import "./styles.css";
import "./themes.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </StrictMode>,
);

// PWA SW only for web builds (api-web sets requiresAuth=true; Tauri desktop does not).
if (requiresAuth && "serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    void navigator.serviceWorker.register("/sw.js").catch(() => {
      /* registration best-effort */
    });
  });
}
