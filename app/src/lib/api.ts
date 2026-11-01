import { invoke } from "@tauri-apps/api/core";

export type Kind = "microphone" | "headset" | "keyboard" | "mouse" | "device";

export interface Device {
  id: string;
  name: string;
  kind: Kind;
  product: string;
  connection: "USB" | "LIGHTSPEED";
  battery: { percent: number; charging: boolean } | null;
  caps: string[];
  features: { id: string; name: string; version: number }[];
}

export type Effect =
  | { kind: "solid"; color: string }
  | { kind: "breathe"; color: string; period_ms: number; intensity: number }
  | { kind: "cycle"; period_ms: number; intensity: number; saturation: number };

export const api = {
  listDevices: () => invoke<Device[]>("list_devices"),
  setEffect: (id: string, effect: Effect) => invoke<void>("set_effect", { id, effect }),
  setBrightness: (id: string, percent: number) => invoke<void>("set_brightness", { id, percent }),
  getGain: (id: string) => invoke<number>("get_gain", { id }),
  setGain: (id: string, gain: number) => invoke<void>("set_gain", { id, gain }),
  setMuted: (id: string, muted: boolean) => invoke<void>("set_muted", { id, muted }),
};

/** Runs `fn` once input has been idle for `ms`, so sliders don't flood the device. */
export function debounce<A extends unknown[]>(fn: (...a: A) => void, ms = 150) {
  let t: ReturnType<typeof setTimeout> | undefined;
  return (...a: A) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...a), ms);
  };
}
