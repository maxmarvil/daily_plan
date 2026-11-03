import { mount } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App.svelte";
import Calendar from "./Calendar.svelte";
import "./app.css";

// одно SPA, разные окна по label
const isCalendar = getCurrentWindow().label === "calendar";

const app = mount(isCalendar ? Calendar : App, {
  target: document.getElementById("app")!,
});

export default app;
