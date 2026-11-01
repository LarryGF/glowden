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

/** Mirrors lhubd's config: speeds and sizes are 1..100 sliders. */
export type Lighting =
  | { effect: "fixed"; color: string }
  | { effect: "breathing"; color: string; speed: number; intensity: number }
  | { effect: "cycle"; speed: number; intensity: number; saturation: number }
  | { effect: "ripple"; color: string }
  | { effect: "wave"; speed: number }
  | { effect: "pulse"; color: string; base: string; speed: number; width: number; fade: number; rainbow: boolean }
  | { effect: "starlight"; star: string; sky: string; density: number }
  | { effect: "visualizer"; low: string; high: string; background: string };

export interface DeviceConfig {
  lighting?: Lighting;
  brightness?: number;
}

const request = <T>(req: Record<string, unknown>) => invoke<T>("request", { req });

export const api = {
  listDevices: () => request<Device[]>({ cmd: "list" }),
  getConfig: (id: string) => request<DeviceConfig>({ cmd: "get_config", id }),
  setLighting: (id: string, lighting: Lighting) => request<void>({ cmd: "set_lighting", id, lighting }),
  setBrightness: (id: string, percent: number) => request<void>({ cmd: "set_brightness", id, percent }),
  release: (id: string) => request<void>({ cmd: "release", id }),
  getGain: (id: string) => request<number>({ cmd: "get_gain", id }),
  setGain: (id: string, gain: number) => request<void>({ cmd: "set_gain", id, gain }),
  setMuted: (id: string, muted: boolean) => request<void>({ cmd: "set_muted", id, muted }),
};

/** Runs `fn` once input has been idle for `ms`, so sliders don't flood the device. */
export function debounce<A extends unknown[]>(fn: (...a: A) => void, ms = 150) {
  let t: ReturnType<typeof setTimeout> | undefined;
  return (...a: A) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...a), ms);
  };
}
