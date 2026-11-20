import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import App from "./App";
import QuickAdd from "./QuickAdd";
import { LanguageProvider } from "./i18n";

const isQuickAdd = getCurrentWebviewWindow().label === "quick-add";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LanguageProvider>
      {isQuickAdd ? <QuickAdd /> : <App />}
    </LanguageProvider>
  </React.StrictMode>,
);
