import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CaptureRoot } from "./windows/capture/CaptureRoot";
import { OrbRoot } from "./windows/orb/OrbRoot";
import { PanelRoot } from "./windows/panel/PanelRoot";
import "./lib/tokens.css";

function WindowRoot() {
  switch (getCurrentWindow().label) {
    case "orb":
      return <OrbRoot />;
    case "capture":
      return <CaptureRoot />;
    case "panel":
    case "settings":
      return <PanelRoot />;
    default:
      return null;
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <WindowRoot />
  </React.StrictMode>,
);
