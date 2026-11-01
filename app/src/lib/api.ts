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
  model: string | null;
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
  dpi_levels?: number[];
  dpi_level?: number;
  report_interval?: number;
}

export interface MouseInfo {
  dpi: number;
  /** Plain values and ranges encoded as low, 0xe000 | step, high. */
  dpi_list: number[];
  report_interval: number;
  report_intervals: number[];
}

/** Device picture with LED shapes in its pixel space (from the imported G HUB metadata). */
export interface Layout {
  width: number;
  height: number;
  image: string;
  shapes: {
    zone: number;
    shape:
      | { type: "rect"; rect: { x: number; y: number; width: number; height: number } }
      | { type: "circle"; rect: { x: number; y: number; r: number } }
      | { type: "path"; d: string };
  }[];
}

export interface LightState {
  mode: "live" | "device" | "none";
  effect: string | null;
  zones: Record<string, string>;
}

const request = <T>(req: Record<string, unknown>) => invoke<T>("request", { req });

export const api = {
  listDevices: () => request<Device[]>({ cmd: "list" }),
  getConfig: (id: string) => request<DeviceConfig>({ cmd: "get_config", id }),
  setLighting: (id: string, lighting: Lighting) => request<void>({ cmd: "set_lighting", id, lighting }),
  setBrightness: (id: string, percent: number) => request<void>({ cmd: "set_brightness", id, percent }),
  release: (id: string) => request<void>({ cmd: "release", id }),
  deviceLayout: (id: string) => request<Layout | null>({ cmd: "device_layout", id }),
  lightingState: (id: string) => request<LightState>({ cmd: "lighting_state", id }),
  deviceImage: (id: string) => request<string | null>({ cmd: "device_image", id }),
  mouseInfo: (id: string) => request<MouseInfo>({ cmd: "mouse_info", id }),
  setDpi: (id: string, levels: number[], level: number) => request<void>({ cmd: "set_dpi", id, levels, level }),
  setReportInterval: (id: string, ms: number) => request<void>({ cmd: "set_report_interval", id, ms }),
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
