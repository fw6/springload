import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { BeginnerModeProvider } from "./copy/BeginnerModeContext";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BeginnerModeProvider>
      <App />
    </BeginnerModeProvider>
  </React.StrictMode>,
);
