/**
 * src/main.tsx
 *
 * Application bootstrap - mounts the React tree into the DOM. Adds a small
 * CSS-ready marker class when the app has rendered once so the window styling
 * (rounded corners / transparency) can be revealed safely.
 */

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

const rootEl = document.getElementById("root") as HTMLElement;
ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
// Mark app as ready so we can reveal transparent corners safely
requestAnimationFrame(() => {
  document.documentElement.classList.add("app-ready");
  rootEl.classList.add("app-ready");
  document.body.classList.add("app-ready");
});
