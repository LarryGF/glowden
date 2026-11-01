//! Physical key positions and the mapping between Linux key codes, HID usages and
//! per-key zone ids (zone = HID usage − 3; modifiers sit at 0x68..=0x6f).

/// HID usage → zone id.
pub fn zone_for_usage(usage: u8) -> Option<u8> {
    match usage {
        0x04..=0x72 => Some(usage - 3),
        0xe0..=0xe7 => Some(0x68 + (usage - 0xe0)),
        _ => None,
    }
}

/// Linux evdev key code → HID keyboard usage, for the keys a TKL board has.
pub fn usage_for_keycode(code: u16) -> Option<u8> {
    Some(match code {
        1 => 0x29,                         // Esc
        2..=10 => 0x1e + (code - 2) as u8, // 1..9
        11 => 0x27,                        // 0
        12 => 0x2d,
        13 => 0x2e,
        14 => 0x2a, // Backspace
        15 => 0x2b, // Tab
        16 => 0x14, // Q
        17 => 0x1a, // W
        18 => 0x08, // E
        19 => 0x15, // R
        20 => 0x17, // T
        21 => 0x1c, // Y
        22 => 0x18, // U
        23 => 0x0c, // I
        24 => 0x12, // O
        25 => 0x13, // P
        26 => 0x2f,
        27 => 0x30,
        28 => 0x28, // Enter
        29 => 0xe0, // LCtrl
        30 => 0x04, // A
        31 => 0x16, // S
        32 => 0x07, // D
        33 => 0x09, // F
        34 => 0x0a, // G
        35 => 0x0b, // H
        36 => 0x0d, // J
        37 => 0x0e, // K
        38 => 0x0f, // L
        39 => 0x33,
        40 => 0x34,
        41 => 0x35, // Grave
        42 => 0xe1, // LShift
        43 => 0x32, // ISO hash / backslash
        44 => 0x1d, // Z
        45 => 0x1b, // X
        46 => 0x06, // C
        47 => 0x19, // V
        48 => 0x05, // B
        49 => 0x11, // N
        50 => 0x10, // M
        51 => 0x36,
        52 => 0x37,
        53 => 0x38,
        54 => 0xe5,                          // RShift
        56 => 0xe2,                          // LAlt
        57 => 0x2c,                          // Space
        58 => 0x39,                          // Caps
        59..=68 => 0x3a + (code - 59) as u8, // F1..F10
        86 => 0x64,                          // ISO extra key
        87 => 0x44,                          // F11
        88 => 0x45,                          // F12
        97 => 0xe4,                          // RCtrl
        99 => 0x46,                          // PrintScreen
        100 => 0xe6,                         // RAlt
        70 => 0x47,                          // ScrollLock
        119 => 0x48,                         // Pause
        110 => 0x49,                         // Insert
        102 => 0x4a,                         // Home
        104 => 0x4b,                         // PageUp
        111 => 0x4c,                         // Delete
        107 => 0x4d,                         // End
        109 => 0x4e,                         // PageDown
        106 => 0x4f,                         // Right
        105 => 0x50,                         // Left
        108 => 0x51,                         // Down
        103 => 0x52,                         // Up
        125 => 0xe3,                         // LMeta
        126 => 0xe7,                         // RMeta
        127 => 0x65,                         // Menu
        _ => return None,
    })
}

/// One row of keys as (HID usage, width in key units); usage 0 is a gap.
type Row = &'static [(u8, f32)];

/// ISO tenkeyless layout, row by row from the top.
const ISO_TKL: [(f32, Row); 6] = [
    (
        0.0,
        &[
            (0x29, 1.0),
            (0, 1.0),
            (0x3a, 1.0),
            (0x3b, 1.0),
            (0x3c, 1.0),
            (0x3d, 1.0),
            (0, 0.5),
            (0x3e, 1.0),
            (0x3f, 1.0),
            (0x40, 1.0),
            (0x41, 1.0),
            (0, 0.5),
            (0x42, 1.0),
            (0x43, 1.0),
            (0x44, 1.0),
            (0x45, 1.0),
            (0, 0.25),
            (0x46, 1.0),
            (0x47, 1.0),
            (0x48, 1.0),
        ],
    ),
    (
        1.25,
        &[
            (0x35, 1.0),
            (0x1e, 1.0),
            (0x1f, 1.0),
            (0x20, 1.0),
            (0x21, 1.0),
            (0x22, 1.0),
            (0x23, 1.0),
            (0x24, 1.0),
            (0x25, 1.0),
            (0x26, 1.0),
            (0x27, 1.0),
            (0x2d, 1.0),
            (0x2e, 1.0),
            (0x2a, 2.0),
            (0, 0.25),
            (0x49, 1.0),
            (0x4a, 1.0),
            (0x4b, 1.0),
        ],
    ),
    (
        2.25,
        &[
            (0x2b, 1.5),
            (0x14, 1.0),
            (0x1a, 1.0),
            (0x08, 1.0),
            (0x15, 1.0),
            (0x17, 1.0),
            (0x1c, 1.0),
            (0x18, 1.0),
            (0x0c, 1.0),
            (0x12, 1.0),
            (0x13, 1.0),
            (0x2f, 1.0),
            (0x30, 1.0),
            (0x28, 1.5),
            (0, 0.25),
            (0x4c, 1.0),
            (0x4d, 1.0),
            (0x4e, 1.0),
        ],
    ),
    (
        3.25,
        &[
            (0x39, 1.75),
            (0x04, 1.0),
            (0x16, 1.0),
            (0x07, 1.0),
            (0x09, 1.0),
            (0x0a, 1.0),
            (0x0b, 1.0),
            (0x0d, 1.0),
            (0x0e, 1.0),
            (0x0f, 1.0),
            (0x33, 1.0),
            (0x34, 1.0),
            (0x32, 1.0),
        ],
    ),
    (
        4.25,
        &[
            (0xe1, 1.25),
            (0x64, 1.0),
            (0x1d, 1.0),
            (0x1b, 1.0),
            (0x06, 1.0),
            (0x19, 1.0),
            (0x05, 1.0),
            (0x11, 1.0),
            (0x10, 1.0),
            (0x36, 1.0),
            (0x37, 1.0),
            (0x38, 1.0),
            (0xe5, 2.75),
            (0, 1.25),
            (0x52, 1.0),
        ],
    ),
    (
        5.25,
        &[
            (0xe0, 1.25),
            (0xe3, 1.25),
            (0xe2, 1.25),
            (0x2c, 6.25),
            (0xe6, 1.25),
            (0xe7, 1.25),
            (0x65, 1.25),
            (0xe4, 1.25),
            (0, 0.25),
            (0x50, 1.0),
            (0x51, 1.0),
            (0x4f, 1.0),
        ],
    ),
];

/// Width and height of the layout in key units.
pub const SIZE: (f32, f32) = (18.25, 6.25);

/// Center position (x, y) of every zone in the ISO TKL layout.
pub fn iso_tkl_positions() -> Vec<(u8, f32, f32)> {
    let mut out = Vec::new();
    for (y, row) in ISO_TKL {
        let mut x = 0.0;
        for &(usage, w) in row {
            if usage != 0
                && let Some(zone) = zone_for_usage(usage)
            {
                out.push((zone, x + w / 2.0, y + 0.5));
            }
            x += w;
        }
    }
    out
}
