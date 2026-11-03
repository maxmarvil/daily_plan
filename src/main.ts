import { mount } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App.svelte";
import Calendar from "./Calendar.svelte";
import TaskView from "./TaskView.svelte";
import Settings from "./Settings.svelte";
import "./app.css";

// одно SPA, разные окна по label
const label = getCurrentWindow().label;
const component =
  label === "calendar"
    ? Calendar
    : label === "settings"
      ? Settings
      : label.startsWith("task-")
        ? TaskView
        : App;

const app = mount(component, {
  target: document.getElementById("app")!,
});

export default app;
