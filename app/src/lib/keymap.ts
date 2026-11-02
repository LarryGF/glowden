/** Browser KeyboardEvent.code → Linux input key name. Both name physical key positions, so
 * a shortcut captured here presses the same key on any layout. */
export function linuxKey(code: string): string | null {
  const m =
    code.match(/^Key([A-Z])$/) ??
    code.match(/^Digit([0-9])$/) ??
    code.match(/^(F[0-9]{1,2})$/);
  if (m) return `KEY_${m[1]}`;
  const np = code.match(/^Numpad([0-9])$/);
  if (np) return `KEY_KP${np[1]}`;
  const table: Record<string, string> = {
    Escape: "ESC", Minus: "MINUS", Equal: "EQUAL", Backspace: "BACKSPACE", Tab: "TAB",
    BracketLeft: "LEFTBRACE", BracketRight: "RIGHTBRACE", Enter: "ENTER", ControlLeft: "LEFTCTRL",
    ControlRight: "RIGHTCTRL", Semicolon: "SEMICOLON", Quote: "APOSTROPHE", Backquote: "GRAVE",
    ShiftLeft: "LEFTSHIFT", ShiftRight: "RIGHTSHIFT", Backslash: "BACKSLASH", Comma: "COMMA",
    Period: "DOT", Slash: "SLASH", AltLeft: "LEFTALT", AltRight: "RIGHTALT", Space: "SPACE",
    CapsLock: "CAPSLOCK", NumLock: "NUMLOCK", ScrollLock: "SCROLLLOCK", NumpadDecimal: "KPDOT",
    NumpadAdd: "KPPLUS", NumpadSubtract: "KPMINUS", NumpadMultiply: "KPASTERISK",
    NumpadDivide: "KPSLASH", NumpadEnter: "KPENTER", IntlBackslash: "102ND", PrintScreen: "SYSRQ",
    Pause: "PAUSE", Home: "HOME", End: "END", PageUp: "PAGEUP", PageDown: "PAGEDOWN",
    Insert: "INSERT", Delete: "DELETE", ArrowUp: "UP", ArrowDown: "DOWN", ArrowLeft: "LEFT",
    ArrowRight: "RIGHT", MetaLeft: "LEFTMETA", MetaRight: "RIGHTMETA", ContextMenu: "COMPOSE",
    AudioVolumeMute: "MUTE", AudioVolumeUp: "VOLUMEUP", AudioVolumeDown: "VOLUMEDOWN",
    MediaPlayPause: "PLAYPAUSE", MediaTrackNext: "NEXTSONG", MediaTrackPrevious: "PREVIOUSSONG",
    MediaStop: "STOPCD",
  };
  return table[code] ? `KEY_${table[code]}` : null;
}

const MODIFIERS = ["KEY_LEFTCTRL", "KEY_RIGHTCTRL", "KEY_LEFTSHIFT", "KEY_RIGHTSHIFT", "KEY_LEFTALT", "KEY_RIGHTALT", "KEY_LEFTMETA", "KEY_RIGHTMETA"];

/** Modifiers first, as a shortcut is pressed. */
export function orderKeys(keys: string[]): string[] {
  return [...keys].sort((a, b) => Number(!MODIFIERS.includes(a)) - Number(!MODIFIERS.includes(b)));
}

/** "KEY_LEFTCTRL" → "Left Ctrl"-style label. */
export function keyLabel(name: string): string {
  const n = name.replace(/^(KEY|BTN)_/, "");
  const special: Record<string, string> = {
    LEFTCTRL: "Ctrl", RIGHTCTRL: "Right Ctrl", LEFTSHIFT: "Shift", RIGHTSHIFT: "Right Shift",
    LEFTALT: "Alt", RIGHTALT: "AltGr", LEFTMETA: "Super", RIGHTMETA: "Right Super", SYSRQ: "Print",
    PLAYPAUSE: "Play/Pause", NEXTSONG: "Next track", PREVIOUSSONG: "Previous track", STOPCD: "Stop",
    VOLUMEUP: "Volume up", VOLUMEDOWN: "Volume down", MUTE: "Mute", MICMUTE: "Mic mute",
    UP: "↑", DOWN: "↓", LEFT: "←", RIGHT: "→",
  };
  const buttons: Record<string, string> = {
    LEFT: "Left click", RIGHT: "Right click", MIDDLE: "Middle click", SIDE: "Back", EXTRA: "Forward",
  };
  if (name.startsWith("BTN_")) return buttons[n] ?? n.charAt(0) + n.slice(1).toLowerCase();
  return special[n] ?? n.charAt(0) + n.slice(1).toLowerCase();
}
