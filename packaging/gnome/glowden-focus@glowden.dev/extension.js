// Reports the focused window's app to the GlowDen service, which switches per-app profiles.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class GlowDenFocus extends Extension {
    enable() {
        this._handler = global.display.connect('notify::focus-window', () => this._report());
        this._report();
    }

    disable() {
        global.display.disconnect(this._handler);
        this._handler = null;
    }

    _report() {
        const w = global.display.focus_window;
        if (!w)
            return;
        // Fire and forget: the service may not be running.
        Gio.DBus.session.call(
            'dev.glowden.Focus', '/focus', 'dev.glowden.Focus', 'Activated',
            new GLib.Variant('(ss)', [w.get_wm_class() ?? '', w.get_title() ?? '']),
            null, Gio.DBusCallFlags.NO_AUTO_START, 1000, null, null);
    }
}
