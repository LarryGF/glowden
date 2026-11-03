import { listen } from "@tauri-apps/api/event";

/** False while the window is in the tray or minimized: live views stop polling and drawing. */
export const page = $state({ visible: !document.hidden });

let shown = true;
const update = () => (page.visible = shown && !document.hidden);
document.addEventListener("visibilitychange", update);
listen<boolean>("window-visible", (e) => {
  shown = e.payload;
  update();
});
