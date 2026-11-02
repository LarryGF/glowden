import { invoke } from "@tauri-apps/api/core";

export type VoiceState = {
  plugin_error: string | null;
  supported: boolean;
  engine: boolean;
  /** G HUB's presets (from the user's G HUB files) and the user's own. */
  presets: string[];
  custom_presets: string[];
  enabled: boolean;
  preset: string | null;
  running: boolean;
  error: string | null;
  has_test: boolean;
  previewing: boolean;
  default_mic: boolean;
  settings: Record<string, unknown>;
  overrides: Record<string, number | boolean>;
  /** From the device's plugin. */
  controls: { path: string; group: string; label: string; min: number; max: number; step: number; unit: string }[];
  switches: { group: string; path: string }[];
  effect_stages: EffectStage[];
  details: Record<string, {
    note?: string;
    noise_reduction: number | null;
    bass: number;
    mids: number;
    treble: number;
    compression: number | null;
    de_esser: number | null;
    boost: number | null;
    input_gain: number | null;
  }>;
  effects: string[];
  custom_effects: string[];
  /** What each effect preset turns on. */
  effect_details: Record<string, string[]>;
  effect: string;
  effect_settings: EffectSettings | null;
  effect_overrides: Record<string, number | boolean | string>;
};

export type EffectStage = {
  stage: string;
  title: string;
  enable?: string;
  controls: (
    | { field: string; label: string; min: number; max: number; step: number; unit: string }
    | { field: string; label: string; modes: { value: string; label: string }[] }
  )[];
};

/** G HUB's voice effect settings: stage → field → value. */
export type EffectSettings = Record<string, Record<string, number | boolean | string>>;

export type Source = {
  url: string;
  builtin: boolean;
  trusted: boolean;
  name: string | null;
  version: string | null;
  fetched: boolean;
  devices: { id: string; name: string; kind: string | null; plugins: string[]; installed: boolean }[];
  plugins: { id: string; name: string; description: string }[];
};

export type GhubSummary = {
  devices: { name: string; found: boolean }[];
  sounds: number;
  voice_presets: number;
  voice_effects: number;
  settings: string | null;
  engine: string | null;
};

export type AppInfo = {
  settings: { ghub_paths: string[]; low_battery_alert: boolean; lighting_off_when_locked: boolean; swap_mouse_buttons: boolean; low_battery_lighting: boolean; lock_notifications: boolean };
  autostart: boolean;
  drivers: { name: string; file: string; products: string[] }[];
  devices: { id: string; name: string; product: string; kind: string; driver: string | null }[];
};

export type Action =
  | { type: "keys"; keys: string[] }
  | { type: "macro"; steps: ({ keys: string[] } | { ms: number })[] }
  | { type: "mouse_button"; button: string }
  | { type: "scroll"; direction: string }
  | { type: "dpi"; action: string }
  | { type: "command"; command: string }
  | { type: "sound"; file: string }
  | { type: "disabled" };

export type ButtonsInfo = {
  labels: Record<string, string>;
  last: { button: number; ago_ms: number } | null;
  assignments: Record<string, Action>;
  keys: string[];
  uinput: string | null;
  sounds: { name: string; file: string }[];
};

export type ProfilesInfo = {
  profiles: { name: string; apps: string[]; devices: string[] }[];
  active: string | null;
  auto: boolean;
  focused: string | null;
  recent: string[];
};

export type Kind = "microphone" | "headset" | "keyboard" | "mouse" | "device";

export interface Device {
  id: string;
  name: string;
  kind: Kind;
  product: string;
  connection: "USB" | "LIGHTSPEED";
  battery: { percent: number; charging: boolean; hours?: number } | null;
  caps: string[];
  features: { id: string; name: string; version: number }[];
  model: string | null;
  firmware_effects: { id: string; label: string }[];
}

/** Mirrors glowdend's config: speeds, sizes and levels are 1..100 sliders. `glow` scales the
 * effect layer and `backdrop` the background of overlay effects. */
export type Lighting =
  | { effect: "fixed"; color: string }
  | { effect: "breathing"; color: string; speed: number; intensity: number }
  | { effect: "cycle"; speed: number; intensity: number; saturation: number }
  | { effect: "firmware"; id: string; color: string; glow: number }
  | { effect: "ripple"; color: string }
  | { effect: "freestyle"; base: string; keys: Record<string, string> }
  | { effect: "wave"; speed: number }
  | { effect: "pulse"; color: string; base: string; speed: number; width: number; fade: number; rainbow: boolean; glow: number; backdrop: number }
  | { effect: "starlight"; star: string; sky: string; density: number; glow: number; backdrop: number }
  | { effect: "visualizer"; low: string; high: string; background: string; glow: number; backdrop: number }
  | { effect: "screen"; speed: number; boost: number }
  | { effect: "sequence"; steps: { color: string; ms: number }[]; blend: boolean; spread: number };

export interface DeviceConfig {
  lighting?: Lighting;
  brightness?: number;
  power?: boolean;
  dpi_levels?: number[];
  dpi_level?: number;
  dpi_default?: number;
  dpi_shift?: number;
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
  /** Keyboards: light only the bright legend pixels of the photo. */
  legends: boolean;
  /** Show only this region of the picture (first view of a multi-view lighting picture). */
  crop: { x: number; y: number; width: number; height: number } | null;
  clip: string | null;
  cutout?: string | null;
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
  voiceState: (id: string) => request<VoiceState>({ cmd: "voice_state", id }),
  setVoice: (id: string, enabled: boolean, preset: string | null, default_mic = true) =>
    request<VoiceState>({ cmd: "set_voice", id, enabled, preset, default_mic }),
  setVoiceOverrides: (id: string, overrides: Record<string, number | boolean>) =>
    request<VoiceState>({ cmd: "set_voice_overrides", id, overrides }),
  getMuted: (id: string) => request<boolean>({ cmd: "get_muted", id }),
  voiceTestRecord: (id: string, seconds: number) => request<void>({ cmd: "voice_test_record", id, seconds }),
  voicePreview: (id: string, on: boolean) => request<VoiceState>({ cmd: "voice_preview", id, on }),
  voicePresetAction: (id: string, kind: "voice" | "effect", action: "duplicate" | "rename" | "delete", name?: string, new_name?: string) =>
    request<VoiceState>({ cmd: "voice_preset_action", id, kind, action, name, new_name }),
  setVoiceEffect: (id: string, effect: string | null) => request<VoiceState>({ cmd: "set_voice_effect", id, effect }),
  setVoiceEffectOverrides: (id: string, overrides: Record<string, number | boolean | string>) =>
    request<VoiceState>({ cmd: "set_voice_effect_overrides", id, overrides }),
  voiceLevels: (id: string) => request<{ input: number; output: number } | null>({ cmd: "voice_levels", id }),
  appSettings: () => request<AppInfo>({ cmd: "app_settings" }),
  profiles: () => request<ProfilesInfo>({ cmd: "profiles" }),
  backups: () => request<string[]>({ cmd: "backups" }),
  lightingPresets: () => request<{ name: string; lighting: Lighting }[]>({ cmd: "lighting_presets" }),
  saveLightingPreset: (name: string, lighting: Lighting) =>
    request<{ name: string; lighting: Lighting }[]>({ cmd: "save_lighting_preset", name, lighting }),
  deleteLightingPreset: (name: string) =>
    request<{ name: string; lighting: Lighting }[]>({ cmd: "delete_lighting_preset", name }),
  exportLightingPresets: () => request<string>({ cmd: "export_lighting_presets" }),
  importLightingPresets: (path: string) =>
    request<{ name: string; lighting: Lighting }[]>({ cmd: "import_lighting_presets", path }),
  saveBackup: () => request<string[]>({ cmd: "save_backup" }),
  restoreBackup: (name: string) => request<string[]>({ cmd: "restore_backup", name }),
  deleteBackup: (name: string) => request<string[]>({ cmd: "delete_backup", name }),
  setActiveProfile: (name: string | null) => request<ProfilesInfo>({ cmd: "set_active_profile", name }),
  saveProfile: (name: string, apps: string[], old: string | null = null) =>
    request<ProfilesInfo>({ cmd: "save_profile", name, apps, old }),
  deleteProfile: (name: string) => request<ProfilesInfo>({ cmd: "delete_profile", name }),
  setProfileAuto: (on: boolean) => request<ProfilesInfo>({ cmd: "set_profile_auto", on }),
  exportProfile: (name: string) => request<string>({ cmd: "export_profile", name }),
  importProfile: (path: string) => request<ProfilesInfo>({ cmd: "import_profile", path }),
  buttons: (id: string) => request<ButtonsInfo>({ cmd: "buttons", id }),
  setAssignment: (id: string, button: number, action: Action | null) =>
    request<void>({ cmd: "set_assignment", id, button, action }),
  setAppSettings: (settings: AppInfo["settings"]) => request<AppInfo>({ cmd: "set_app_settings", settings }),
  setAutostart: (on: boolean) => request<boolean>({ cmd: "set_autostart", on }),
  sources: () => request<Source[]>({ cmd: "sources" }),
  refreshSources: () => request<{ sources: Source[]; errors: string[] }>({ cmd: "refresh_sources" }),
  addSource: (url: string) => request<Source[]>({ cmd: "add_source", url }),
  removeSource: (url: string) => request<Source[]>({ cmd: "remove_source", url }),
  trustSource: (url: string, trusted: boolean) => request<Source[]>({ cmd: "trust_source", url, trusted }),
  installDevice: (url: string, id: string) => request<Source[]>({ cmd: "install_device", url, id }),
  uninstallDevice: (url: string, id: string) => request<Source[]>({ cmd: "uninstall_device", url, id }),
  scanGhub: (rescan = true) => request<GhubSummary>({ cmd: "scan_ghub", rescan }),
  createDriver: (id: string) => request<string>({ cmd: "create_driver", id }),
  getConfig: (id: string) => request<DeviceConfig>({ cmd: "get_config", id }),
  setLighting: (id: string, lighting: Lighting) => request<void>({ cmd: "set_lighting", id, lighting }),
  setBrightness: (id: string, percent: number) => request<void>({ cmd: "set_brightness", id, percent }),
  setPower: (id: string, on: boolean) => request<void>({ cmd: "set_power", id, on }),
  syncLighting: (from: string) => request<string[]>({ cmd: "sync_lighting", from }),
  release: (id: string) => request<void>({ cmd: "release", id }),
  deviceLayout: (id: string) => request<Layout | null>({ cmd: "device_layout", id }),
  lightingState: (id: string) => request<LightState>({ cmd: "lighting_state", id }),
  deviceImage: (id: string) => request<string | null>({ cmd: "device_image", id }),
  mouseInfo: (id: string) => request<MouseInfo>({ cmd: "mouse_info", id }),
  setDpi: (id: string, levels: number[], level: number, dflt: number | null = null, shift: number | null = null) =>
    request<void>({ cmd: "set_dpi", id, levels, level, default: dflt, shift }),
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
