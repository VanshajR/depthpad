"""Depthpad desktop app."""
import copy
import json
import logging
import os
import sys
import time
import webbrowser

from PySide6.QtCore import QPoint, QTimer, Qt, QUrl
from PySide6.QtGui import QDesktopServices, QFont, QIcon
from PySide6.QtNetwork import QLocalServer, QLocalSocket, QNetworkAccessManager, QNetworkRequest
from PySide6.QtWidgets import (QApplication, QButtonGroup, QComboBox, QDoubleSpinBox, QFileDialog, QFrame,
                               QGridLayout, QHBoxLayout, QInputDialog, QLabel, QMainWindow, QMenu, QMessageBox,
                               QPushButton, QScrollArea, QSlider, QStackedWidget, QSystemTrayIcon, QVBoxLayout,
                               QWidget)

from ak680.engine import Engine
from ak680.profile import (ANALOG_TARGETS, BUTTON_TARGETS, BUILTIN, CURVE_DEFAULTS, MAX_TRAVEL_MM, ProfileStore,
                           apply_curve, curve_of, validate)
from ak680.protocol import load_keymap
from ak680.rawinput import SCAN_TO_HID

from . import APP_NAME, REPO, VERSION, appdata
from . import theme as T
from .layout import LEGEND, TARGETS_LONG
from .widgets import (AddCard, AxisMeter, Banner, ControllerView, CurveView, Group, IconButton, KeyboardView, Legend,
                      LiveChart, Panel, ProfileCard, RailButton, Row, Segmented, SliderField, StatusDot,
                      TargetPicker, Toggle, app_icon_pixmap, chevron_png, shadow, text)

log = logging.getLogger("depthpad")
INSTANCE_KEY = "Depthpad-single-instance"
VIGEM_URL = "https://github.com/nefarius/ViGEmBus/releases/latest"


def app_icon():
    icon = QIcon()
    for s in (16, 20, 24, 32, 40, 48, 64, 128, 256):
        icon.addPixmap(app_icon_pixmap(s))
    return icon


class CompactValue(QWidget):
    """Slider + editable number + unit on one line (for settings rows)."""

    def __init__(self, lo, hi, step, unit, on_change, decimals=2):
        super().__init__()
        self.lo, self.step = lo, step
        h = QHBoxLayout(self)
        h.setContentsMargins(0, 0, 0, 0)
        h.setSpacing(10)
        self.slider = QSlider(Qt.Horizontal)
        self.slider.setRange(0, round((hi - lo) / step))
        self.slider.setFixedWidth(170)
        self.box = QDoubleSpinBox()
        self.box.setRange(lo, hi)
        self.box.setSingleStep(step)
        self.box.setDecimals(decimals)
        self.box.setAlignment(Qt.AlignRight)
        self.box.setFixedWidth(64)
        self.box.setFont(T.font(T.LABEL, QFont.DemiBold, tabular=True))
        h.addWidget(self.slider)
        h.addWidget(self.box)
        u = text(unit, "Unit")
        u.setFixedWidth(24)
        h.addWidget(u)
        self.on_change = on_change
        self.slider.valueChanged.connect(lambda i: self._set(round(lo + i * step, 4), from_slider=True))
        self.box.valueChanged.connect(lambda v: self._set(round(v, 4)))

    def _set(self, v, from_slider=False):
        w = self.box if from_slider else self.slider
        w.blockSignals(True)
        if from_slider:
            self.box.setValue(v)
        else:
            self.slider.setValue(round((v - self.lo) / self.step))
        w.blockSignals(False)
        self.on_change(v)

    def setValue(self, v):
        for w in (self.slider, self.box):
            w.blockSignals(True)
        self.box.setValue(v)
        self.slider.setValue(round((v - self.lo) / self.step))
        for w in (self.slider, self.box):
            w.blockSignals(False)

    def value(self):
        return round(self.box.value(), 4)


class Page(QScrollArea):
    def __init__(self, title=None, subtitle=None):
        super().__init__()
        self.setWidgetResizable(True)
        self.setFrameShape(QFrame.NoFrame)
        inner = QWidget()
        inner.setObjectName("Plane")
        self.lay = QVBoxLayout(inner)
        self.lay.setContentsMargins(28, 6, 28, 28)
        self.lay.setSpacing(16)
        if title:
            head = QVBoxLayout()
            head.setSpacing(4)
            head.addWidget(text(title, "Title"))
            if subtitle:
                head.addWidget(text(subtitle, "Secondary"))
            self.lay.addLayout(head)
            self.lay.addSpacing(4)
        self.setWidget(inner)

    def group(self, title):
        self.lay.addSpacing(4)
        self.lay.addWidget(text(title, "Heading"))
        g = Group()
        self.lay.addWidget(g)
        return g


# --------------------------------------------------------------------------------------------- live
class AnalysisView(QWidget):
    def __init__(self):
        super().__init__()
        v = QVBoxLayout(self)
        v.setContentsMargins(0, 0, 0, 0)
        v.setSpacing(18)
        meters = QGridLayout()
        meters.setHorizontalSpacing(22)
        self.meters = [AxisMeter("Left stick X", True), AxisMeter("Left stick Y", True),
                       AxisMeter("Right stick X", True), AxisMeter("Right stick Y", True),
                       AxisMeter("Left trigger", False), AxisMeter("Right trigger", False)]
        for i, m in enumerate(self.meters):
            meters.addWidget(m, 0, i)
        v.addLayout(meters)
        charts = QHBoxLayout()
        charts.setSpacing(22)
        s1, s2 = T.SERIES[0], T.SERIES[1]
        self.charts = [LiveChart("Left stick", [("X", s1), ("Y", s2)], -1, 1),
                       LiveChart("Right stick", [("X", s1), ("Y", s2)], -1, 1),
                       LiveChart("Triggers", [("LT", s1), ("RT", s2)], 0, 1)]
        for c in self.charts:
            charts.addWidget(c)
        v.addLayout(charts, 1)
        v.addWidget(text("Last 5 seconds. Hover a chart to read values.", "Tertiary"))

    def push(self, t, out):
        b, lt, rt, (lx, ly, rx, ry) = out
        for m, val in zip(self.meters, (lx, ly, rx, ry, lt, rt)):
            m.set_value(val)
        self.charts[0].push(t, (lx, ly))
        self.charts[1].push(t, (rx, ry))
        self.charts[2].push(t, (lt, rt))


class LivePage(Page):
    def __init__(self, keymap):
        super().__init__()
        kb = Panel("Keyboard", Legend([(T.ACCENT, "Measured depth"), (T.HELD, "Held from key press")]))
        self.keyboard = KeyboardView(keymap)
        kb.body.addWidget(self.keyboard, 0, Qt.AlignHCenter)
        self.lay.addWidget(kb)

        self.mode = Segmented(["Controller", "Analysis"])
        out = Panel("Output", self.mode)
        self.views = QStackedWidget()
        self.controller = ControllerView()
        self.analysis = AnalysisView()
        self.views.addWidget(self.controller)
        self.views.addWidget(self.analysis)
        self.views.setMinimumHeight(290)
        self.mode.changed.connect(self.views.setCurrentIndex)
        out.body.addWidget(self.views, 1)
        self.lay.addWidget(out, 1)

    def update_state(self, s, bindings):
        self.keyboard.set_bindings(bindings)
        self.keyboard.set_state(s["keys"])
        self.controller.set_state(s["output"])


# --------------------------------------------------------------------------------------------- mapping
class MappingPage(Page):
    def __init__(self, keymap, on_change):
        super().__init__()
        self.keymap = keymap
        self.on_change = on_change
        self.profile = None
        self.pos = None
        self.by_name = {v["key"]: p for p, v in keymap.items()}

        cols = QHBoxLayout()
        cols.setSpacing(16)
        left = QVBoxLayout()
        left.setSpacing(16)
        actions = QWidget()
        ah = QHBoxLayout(actions)
        ah.setContentsMargins(0, 0, 0, 0)
        ah.setSpacing(4)
        self.reset_btn = QPushButton("Reset to defaults")
        self.clear_btn = QPushButton("Clear all")
        for b in (self.reset_btn, self.clear_btn):
            b.setObjectName("Quiet")
            b.setCursor(Qt.PointingHandCursor)
            ah.addWidget(b)
        kb = Panel("Click a key, or drag an input onto it", actions)
        self.keyboard = KeyboardView(keymap, selectable=True)
        self.keyboard.keyClicked.connect(self.select)
        self.keyboard.dropped.connect(self._dropped)
        self.keyboard.keyHovered.connect(lambda pos: self.picker.set_hint(self._bindings().get(pos)))
        kb.body.addWidget(self.keyboard, 0, Qt.AlignHCenter)
        left.addWidget(kb)
        resp = Panel("Response")
        self.curve_view = CurveView()
        resp.body.addWidget(self.curve_view, 1)
        self.curve_note = text("", "Tertiary", wrap=True)
        resp.body.addWidget(self.curve_note)
        left.addWidget(resp, 1)
        cols.addLayout(left, 1)

        insp = Panel(margins=(20, 20, 20, 20), spacing=16)
        insp.setFixedWidth(340)
        insp.body.addWidget(text("KEY", "Overline"))
        self.key_title = text("None selected", "Title")
        insp.body.addWidget(self.key_title)
        self.key_target = text("Click a key on the keyboard to map it.", "Secondary", wrap=True)
        insp.body.addWidget(self.key_target)
        insp.body.addSpacing(4)
        self.picker = TargetPicker()
        self.picker.chosen.connect(self._target_chosen)
        self.picker.hovered.connect(lambda t: self.keyboard.set_highlight({t} if t else ()))
        insp.body.addWidget(self.picker)
        insp.body.addSpacing(6)
        resp_head = QHBoxLayout()
        resp_head.addWidget(text("RESPONSE", "Overline"))
        resp_head.addStretch(1)
        self.custom = Segmented(["Default", "Custom"])
        self.custom.changed.connect(lambda i: self._custom_set(i == 1))
        resp_head.addWidget(self.custom)
        insp.body.addLayout(resp_head)
        self.sliders = {
            "deadzone_mm": SliderField("Dead zone", 0, 1.5, 0.05),
            "full_mm": SliderField("Full output at", 0.5, MAX_TRAVEL_MM, 0.05),
            "shape": SliderField("Response shape", 0.3, 3.0, 0.05, unit=""),
            "press_mm": SliderField("Press point", 0.2, 3.2, 0.05),
        }
        for k, s in self.sliders.items():
            s.valueChanged.connect(lambda v, k=k: self._curve_changed(k, v))
            insp.body.addWidget(s)
        insp.body.addStretch(1)
        cols.addWidget(insp)
        self.lay.addLayout(cols, 1)
        self._load_key()

    def set_profile(self, profile):
        self.profile = profile
        self.keyboard.set_bindings(self._bindings())
        self._load_key()

    def _bindings(self):
        out = {}
        for b in self.profile["bindings"]:
            pos = self.by_name.get(b["key"])
            if pos is not None:
                out.setdefault(pos, b["target"])
        return out

    def _binding(self):
        if self.pos is None or self.profile is None:
            return None
        name = self.keymap[self.pos]["key"]
        return next((b for b in self.profile["bindings"] if b["key"] == name), None)

    def select(self, pos):
        self.pos = pos
        self.keyboard.set_selected(pos)
        self._load_key()

    def _load_key(self):
        sends = self.pos is not None and bool(self.keymap[self.pos]["hid"])
        self.picker.setEnabled(sends)
        b = self._binding()
        target = b["target"] if b else None
        custom = bool(b) and any(k in b for k in CURVE_DEFAULTS)
        if self.pos is None:
            self.key_title.setText("None selected")
            self.key_target.setText("Click a key on the keyboard to map it.")
        else:
            self.key_title.setText(LEGEND.get(self.pos) or self.keymap[self.pos]["key"])
            if not sends:
                self.key_target.setText("Fn works inside the keyboard and can't be mapped.")
            else:
                self.key_target.setText(f"Controls: {TARGETS_LONG[target]}" if target else "Not mapped")
        self.picker.set_value(target)
        self.custom.set_index(1 if custom else 0, animate=False)
        curve = curve_of(self.profile, b or {}) if self.profile else dict(CURVE_DEFAULTS)
        for k, s in self.sliders.items():
            s.setValue(curve[k])
        self._refresh(target, custom)

    def _refresh(self, target, custom):
        analog, button = target in ANALOG_TARGETS, target in BUTTON_TARGETS
        self.custom.setVisible(target is not None)
        for k in ("deadzone_mm", "full_mm", "shape"):
            self.sliders[k].setVisible(analog and custom)
        self.sliders["press_mm"].setVisible(button and custom)
        if self.profile is None:
            return
        b = self._binding()
        curve = curve_of(self.profile, b or {})
        self.curve_view.set_curve(curve, lambda d: apply_curve(d, curve), press=button)
        d = self.profile["curve"]
        if target is None:
            note = "Select a mapped key to see its response."
        elif custom:
            note = "This key has its own response."
        elif analog:
            note = (f"Profile default: dead zone {d['deadzone_mm']:.2f} mm, full output at {d['full_mm']:.2f} mm. "
                    "Change the default in Settings, or choose Custom for this key.")
        else:
            note = f"Profile default: the button presses at {d['press_mm']:.2f} mm."
        self.curve_note.setText(note)

    def _dropped(self, pos, target, src):
        """An input dropped on a key: map it there. Dragged from another key: move it, response included."""
        name = self.keymap[pos]["key"]
        curve = {}
        if src is not None and src != pos:
            src_name = self.keymap[src]["key"]
            moved = next((b for b in self.profile["bindings"] if b["key"] == src_name and b["target"] == target), None)
            if moved:
                curve = {k: v for k, v in moved.items() if k in CURVE_DEFAULTS}
            self.profile["bindings"] = [b for b in self.profile["bindings"] if b["key"] != src_name]
        elif src == pos:
            return
        self.profile["bindings"] = [b for b in self.profile["bindings"] if b["key"] != name]
        self.profile["bindings"].append({"key": name, "target": target, **curve})
        self.keyboard.set_bindings(self._bindings())
        self.select(pos)
        self.on_change()

    def _target_chosen(self, t):
        if self.pos is None or not self.keymap[self.pos]["hid"]:
            self._load_key()
            return
        name = self.keymap[self.pos]["key"]
        old = self._binding()
        self.profile["bindings"] = [b for b in self.profile["bindings"] if b["key"] != name]
        if t:
            nb = {"key": name, "target": t}
            if old:
                nb.update({k: v for k, v in old.items() if k in CURVE_DEFAULTS})
            self.profile["bindings"].append(nb)
        self.keyboard.set_bindings(self._bindings())
        self._load_key()
        self.on_change()

    def _custom_set(self, on):
        b = self._binding()
        if not b:
            return
        for k in CURVE_DEFAULTS:
            if on:
                b[k] = self.profile["curve"][k]
            else:
                b.pop(k, None)
        self._load_key()
        self.on_change()

    def _curve_changed(self, key, v):
        b = self._binding()
        if not b or self.custom.index != 1:
            return
        if key == "deadzone_mm" and v >= b.get("full_mm", 99) - 0.1:
            v = round(b["full_mm"] - 0.1, 2)
            self.sliders[key].setValue(v)
        if key == "full_mm" and v <= b.get("deadzone_mm", 0) + 0.1:
            v = round(b["deadzone_mm"] + 0.1, 2)
            self.sliders[key].setValue(v)
        b[key] = v
        self._refresh(b["target"], True)
        self.on_change()

    def update_state(self, s):
        self.keyboard.set_state(s["keys"])
        if self.pos is not None:
            self.curve_view.set_live(s["keys"].get(self.pos, (0.0, None))[0] * MAX_TRAVEL_MM)


# --------------------------------------------------------------------------------------------- settings
class HotkeyButton(QPushButton):
    """Click, then press a key combination (e.g. Right Ctrl + P)."""

    NAMES = {"ControlRight": "Right Ctrl", "ControlLeft": "Left Ctrl", "ShiftLeft": "Left Shift",
             "ShiftRight": "Right Shift", "AltLeft": "Left Alt", "AltRight": "Right Alt", "MetaLeft": "Win"}

    def __init__(self, keymap, on_set):
        super().__init__()
        self.on_set = on_set
        self.hid_to_name = {v["hid"]: v["key"] for v in keymap.values() if v["hid"]}
        self.recording = False
        self.keys = []
        self.setMinimumWidth(180)
        self.setCursor(Qt.PointingHandCursor)
        self.clicked.connect(self.start)

    def pretty(self, value):
        if not value:
            return "None"
        out = []
        for part in value.split("+"):
            if part in self.NAMES:
                out.append(self.NAMES[part])
            elif part.startswith("Key"):
                out.append(part[3:])
            elif part.startswith("Digit"):
                out.append(part[5:])
            else:
                out.append(part)
        return " + ".join(out)

    def set_value(self, value):
        self.setText(self.pretty(value or ""))

    def start(self):
        self.recording = True
        self.keys = []
        self.setText("Press keys…")
        self.grabKeyboard()

    def keyPressEvent(self, e):
        if not self.recording:
            return super().keyPressEvent(e)
        if e.key() == Qt.Key_Escape:
            self._finish("")
            return
        sc = e.nativeScanCode()
        name = self.hid_to_name.get(SCAN_TO_HID.get((sc & 0xFF) | (0xE000 if sc & 0x100 else 0)))
        if name and name not in self.keys and not e.isAutoRepeat():
            self.keys.append(name)
            self.setText(self.pretty("+".join(self.keys)) + " …")

    def keyReleaseEvent(self, e):
        if self.recording and self.keys and not e.isAutoRepeat():
            self._finish("+".join(self.keys[:3]))

    def _finish(self, value):
        self.recording = False
        self.releaseKeyboard()
        self.set_value(value)
        self.on_set(value)


class SettingsPage(Page):
    def __init__(self, keymap, window):
        super().__init__("Settings", "These apply to the active profile, except App.")
        self.w = window
        g = self.group("Sticks")
        self.shape = QComboBox()
        self.shape.addItem("Square", "square")
        self.shape.addItem("Circle", "circle")
        self.shape.setFixedWidth(150)
        self.shape.currentIndexChanged.connect(lambda _: self._set("stick_shape", self.shape.currentData()))
        self.lkp, self.snappy = Toggle(), Toggle()
        self.lkp.toggled.connect(lambda v: self._set("last_key_priority", v))
        self.snappy.toggled.connect(lambda v: self._set("snappy_joystick", v))
        g.add(Row("Stick shape", "Square treats each axis on its own. Circle keeps diagonals at the stick's edge.",
                  self.shape))
        g.add(Row("Last key priority", "With opposite keys held, the newer one wins instead of cancelling out.",
                  self.lkp))
        g.add(Row("Snappy stick", "Opposite keys give the deeper key's full value instead of the difference.",
                  self.snappy))

        g = self.group("Default response")
        self.defaults = {}
        for k, title, desc, lo, hi, unit in (
                ("deadzone_mm", "Dead zone", "Travel ignored at the top of a key.", 0, 1.5, "mm"),
                ("full_mm", "Full output at", "Key travel that counts as 100% for sticks and triggers.", 0.5,
                 MAX_TRAVEL_MM, "mm"),
                ("shape", "Response shape", "1 is linear. Lower gives more output early; higher, finer control.",
                 0.3, 3.0, ""),
                ("press_mm", "Button press point", "Travel at which a mapped button counts as pressed.", 0.2, 3.2,
                 "mm")):
            self.defaults[k] = CompactValue(lo, hi, 0.05, unit, lambda v, k=k: self._set_default(k, v))
            g.add(Row(title, desc, self.defaults[k]))

        g = self.group("Keyboard input")
        self.block, self.alttab = Toggle(), Toggle()
        self.block.toggled.connect(lambda v: self._set("block_keys", v))
        self.alttab.toggled.connect(lambda v: self._set("block_allow_alt_tab", v))
        self.hotkey = HotkeyButton(keymap, lambda v: self._set("pause_hotkey", v))
        g.add(Row("Block mapped keys", "Games see only the controller, not the keys. Applies to every keyboard "
                  "while on, because Windows doesn't say which keyboard a key came from.", self.block))
        g.add(Row("Let Alt and Tab through", "Keep Alt+Tab working while blocking.", self.alttab))
        g.add(Row("Pause hotkey", "Pauses the controller and blocking. Click, then press the keys.", self.hotkey))

        g = self.group("App")
        self.autostart = Toggle(appdata.autostart_enabled())
        self.autostart.toggled.connect(appdata.set_autostart)
        self.minimized = Toggle(self.w.prefs["start_minimized"])
        self.minimized.toggled.connect(lambda v: self.w.set_pref("start_minimized", v))
        self.tray = Toggle(self.w.prefs["close_to_tray"])
        self.tray.toggled.connect(lambda v: self.w.set_pref("close_to_tray", v))
        self.updates = Toggle(self.w.prefs["check_updates"])
        self.updates.toggled.connect(lambda v: self.w.set_pref("check_updates", v))
        g.add(Row("Start with Windows", None, self.autostart))
        g.add(Row("Start in the tray", None, self.minimized))
        g.add(Row("Keep running when the window is closed", "Quit from the tray icon.", self.tray))
        g.add(Row("Check for updates", "Once at start-up.", self.updates))
        folder = QPushButton("Open folder")
        folder.clicked.connect(lambda: QDesktopServices.openUrl(QUrl.fromLocalFile(appdata.PROFILE_DIR)))
        g.add(Row("Profile files", appdata.PROFILE_DIR, folder))
        self.lay.addStretch(1)

    def set_profile(self, p):
        s = p["settings"]
        self.shape.blockSignals(True)
        self.shape.setCurrentIndex(1 if s.get("stick_shape") == "circle" else 0)
        self.shape.blockSignals(False)
        self.lkp.setChecked(s.get("last_key_priority", False))
        self.snappy.setChecked(s.get("snappy_joystick", False))
        self.block.setChecked(s.get("block_keys", False))
        self.alttab.setChecked(s.get("block_allow_alt_tab", True))
        self.hotkey.set_value(s.get("pause_hotkey", ""))
        for k, w in self.defaults.items():
            w.setValue(p["curve"][k])

    def _set_default(self, key, v):
        c = self.w.profile["curve"]
        if key == "deadzone_mm" and v >= c["full_mm"] - 0.1:
            v = round(c["full_mm"] - 0.1, 2)
            self.defaults[key].setValue(v)
        if key == "full_mm" and v <= c["deadzone_mm"] + 0.1:
            v = round(c["deadzone_mm"] + 0.1, 2)
            self.defaults[key].setValue(v)
        c[key] = v
        self.w.profile_changed()
        self.w.mapping._load_key()

    def _set(self, key, value):
        self.w.profile["settings"][key] = value
        self.w.profile_changed()


class AboutPage(Page):
    def __init__(self, window):
        super().__init__()
        self.w = window
        hero = Panel(margins=(24, 24, 24, 24))
        h = QHBoxLayout()
        h.setSpacing(20)
        icon = QLabel()
        icon.setPixmap(app_icon_pixmap(72))
        h.addWidget(icon, 0, Qt.AlignTop)
        v = QVBoxLayout()
        v.setSpacing(4)
        v.addWidget(text(APP_NAME, "Title"))
        v.addWidget(text(f"Version {VERSION}", "Secondary"))
        v.addSpacing(6)
        v.addWidget(text("Analog game controller for the AJAZZ AK680 V2 HE. Free and open source.", "Secondary",
                         wrap=True))
        v.addSpacing(10)
        links = QHBoxLayout()
        links.setSpacing(8)
        for label_text, url in (("Website", f"https://github.com/{REPO}"),
                                ("Report a problem", f"https://github.com/{REPO}/issues")):
            b = QPushButton(label_text)
            b.clicked.connect(lambda _=None, u=url: webbrowser.open(u))
            links.addWidget(b)
        links.addStretch(1)
        v.addLayout(links)
        h.addLayout(v, 1)
        hero.body.addLayout(h)
        self.lay.addWidget(hero)

        g = self.group("Diagnostics")
        self.record = QPushButton("Start recording")
        self.record.clicked.connect(self.w.toggle_recording)
        g.add(Row("Record a session", "To check for stuck or missing input, or to attach to a bug report. "
                  "Recordings contain every key pressed on the keyboard.", self.record))
        logs = QPushButton("Open folder")
        logs.clicked.connect(lambda: QDesktopServices.openUrl(QUrl.fromLocalFile(appdata.LOG_DIR)))
        g.add(Row("Logs and recordings", appdata.LOG_DIR, logs))

        g = self.group("Licences")
        body = text("Depthpad is free software under the GNU AGPL-3.0. Its stick behaviour, key-blocking rules and "
                    "keyboard session handling are ported from HallJoy (PashOK7, AGPL-3.0). The virtual controller "
                    "uses ViGEmClient (MIT) and the ViGEmBus driver by Nefarius. The interface uses Qt (LGPL-3.0) "
                    "and Lucide icons (ISC). AJAZZ, Xbox and Windows are trademarks of their owners; Depthpad is "
                    "not affiliated with them.", "Secondary", wrap=True)
        wrap = QWidget()
        box = QVBoxLayout(wrap)
        box.setContentsMargins(0, 14, 0, 14)
        box.addWidget(body)
        g.add(wrap)
        self.lay.addStretch(1)


# --------------------------------------------------------------------------------------------- window
class MainWindow(QMainWindow):
    PAGES = [("Live", "activity"), ("Mapping", "keyboard"), ("Settings", "sliders-horizontal"), ("About", "info")]

    def __init__(self, prefs):
        super().__init__()
        self.prefs = prefs
        self.keymap = load_keymap()
        self.key_names = {v["key"] for v in self.keymap.values()}
        self.store = ProfileStore(appdata.PROFILE_DIR)
        names = self.store.names()
        self.profile = self.store.load(prefs["profile"] if prefs["profile"] in names else names[0])
        self.recording = None
        self.quitting = False
        self.contention_banner = None
        self.t0 = time.perf_counter()

        self.setWindowTitle(APP_NAME)
        self.setWindowIcon(app_icon())
        self.resize(1280, 860)
        self.setMinimumSize(1080, 720)

        root = QWidget()
        root.setObjectName("Root")
        self.setCentralWidget(root)
        outer = QHBoxLayout(root)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)
        outer.addWidget(self._rail())

        plane = QWidget()
        plane.setObjectName("Plane")
        main = QVBoxLayout(plane)
        main.setContentsMargins(0, 0, 0, 0)
        main.setSpacing(0)
        main.addWidget(self._topbar())
        self.banners = QVBoxLayout()
        self.banners.setContentsMargins(28, 0, 28, 8)
        self.banners.setSpacing(8)
        main.addLayout(self.banners)
        self.stack = QStackedWidget()
        self.live = LivePage(self.keymap)
        self.mapping = MappingPage(self.keymap, self.profile_changed)
        self.mapping.clear_btn.clicked.connect(self.clear_mappings)
        self.mapping.reset_btn.clicked.connect(self.reset_mappings)
        self.settings = SettingsPage(self.keymap, self)
        self.about = AboutPage(self)
        for p in (self.live, self.mapping, self.settings, self.about):
            self.stack.addWidget(p)
        main.addWidget(self.stack, 1)
        outer.addWidget(plane, 1)

        self.engine = Engine(copy.deepcopy(self.profile))
        self.engine.start()
        self._load_profile_into_ui()

        self.save_timer = QTimer(self, singleShot=True, interval=400)
        self.save_timer.timeout.connect(lambda: self.store.save(self.profile))
        self.tick = QTimer(self, interval=16)
        self.tick.timeout.connect(self._tick)
        self.tick.start()
        self._tray()
        self._checks()
        self._show_page(0)

    # --- chrome ------------------------------------------------------------------------------
    def _rail(self):
        rail = QFrame()
        rail.setObjectName("Rail")
        rail.setFixedWidth(76)
        v = QVBoxLayout(rail)
        v.setContentsMargins(8, 18, 8, 16)
        v.setSpacing(6)
        logo = QLabel()
        logo.setPixmap(app_icon_pixmap(36))
        logo.setToolTip(f"{APP_NAME} {VERSION}")
        v.addWidget(logo, 0, Qt.AlignHCenter)
        v.addSpacing(18)
        self.nav_group = QButtonGroup(self)
        self.nav = []
        for i, (label_text, icon_name) in enumerate(self.PAGES):
            b = RailButton(label_text, icon_name)
            self.nav_group.addButton(b, i)
            b.clicked.connect(lambda _=None, i=i: self._show_page(i))
            v.addWidget(b, 0, Qt.AlignHCenter)
            self.nav.append(b)
        v.addStretch(1)
        return rail

    def _topbar(self):
        bar = QWidget()
        h = QHBoxLayout(bar)
        h.setContentsMargins(28, 18, 28, 14)
        h.setSpacing(14)
        self.strip = QHBoxLayout()
        self.strip.setSpacing(10)
        strip_host = QWidget()
        strip_host.setLayout(self.strip)
        scroller = QScrollArea()
        scroller.setWidget(strip_host)
        scroller.setWidgetResizable(True)
        scroller.setFrameShape(QFrame.NoFrame)
        scroller.setFixedHeight(66)
        scroller.setVerticalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        h.addWidget(scroller, 1)
        status = QVBoxLayout()
        status.setSpacing(0)
        self.kb_status = StatusDot()
        self.pad_status = StatusDot()
        status.addWidget(self.kb_status)
        status.addWidget(self.pad_status)
        h.addLayout(status)
        self.pause_btn = IconButton("pause", "Pause the controller", size=38)
        self.pause_btn.clicked.connect(lambda: self.engine.set_paused(not self.engine.paused))
        h.addWidget(self.pause_btn)
        return bar

    def _rebuild_strip(self):
        while self.strip.count():
            item = self.strip.takeAt(0)
            if item.widget():
                item.widget().deleteLater()
        for name in self.store.names():
            card = ProfileCard(name, name == self.profile["name"])
            card.clicked.connect(self.switch_profile)
            card.menu.connect(self._profile_menu)
            self.strip.addWidget(card)
        add = AddCard()
        add.clicked.connect(self._add_menu)
        self.strip.addWidget(add)
        self.strip.addStretch(1)

    def _profile_menu(self, name, pos):
        if name != self.profile["name"]:
            self.switch_profile(name)
        m = QMenu(self)
        m.addAction("Rename…", self.rename_profile)
        m.addAction("Duplicate", self.duplicate_profile)
        m.addAction("Export…", self.export_profile)
        m.addSeparator()
        m.addAction("Delete", self.delete_profile)
        m.exec(QPoint(int(pos.x()), int(pos.y())))

    def _add_menu(self):
        m = QMenu(self)
        m.addAction("New profile", self.new_profile)
        m.addAction("Duplicate current", self.duplicate_profile)
        m.addAction("Import…", self.import_profile)
        m.exec(self.cursor().pos())

    def _show_page(self, i):
        self.nav[i].setChecked(True)
        self.stack.setCurrentIndex(i)

    def _tray(self):
        self.tray = QSystemTrayIcon(app_icon(), self)
        self.tray.setToolTip(APP_NAME)
        menu = QMenu()
        menu.addAction("Open Depthpad", self.show_window)
        self.tray_pause = menu.addAction("Pause", lambda: self.engine.set_paused(not self.engine.paused))
        menu.addSeparator()
        menu.addAction("Quit", self.quit)
        self.tray.setContextMenu(menu)
        self.tray.activated.connect(lambda r: self.show_window() if r == QSystemTrayIcon.Trigger else None)
        self.tray.show()

    def _checks(self):
        if not appdata.vigembus_installed():
            btn = QPushButton("Get ViGEmBus")
            btn.setObjectName("Primary")
            btn.clicked.connect(lambda: webbrowser.open(VIGEM_URL))
            self.banners.addWidget(Banner("The ViGEmBus driver isn't installed, so games can't see the controller. "
                                          "Install it, then restart Depthpad.", error=True, action=btn))
        if self.prefs["check_updates"]:
            self.net = QNetworkAccessManager(self)
            req = QNetworkRequest(QUrl(f"https://api.github.com/repos/{REPO}/releases/latest"))
            req.setRawHeader(b"Accept", b"application/vnd.github+json")
            reply = self.net.get(req)
            reply.finished.connect(lambda: self._update_reply(reply))

    def _update_reply(self, reply):
        data, newer, tag = {}, False, ""
        try:
            data = json.loads(bytes(reply.readAll()).decode("utf8"))
            tag = str(data.get("tag_name", "")).lstrip("v")
            newer = bool(tag) and tuple(int(x) for x in tag.split(".")[:3]) > tuple(int(x) for x in VERSION.split("."))
        except (ValueError, TypeError, AttributeError):
            newer = False
        reply.deleteLater()
        if newer:
            btn = QPushButton("Download")
            btn.setObjectName("Primary")
            btn.clicked.connect(lambda: webbrowser.open(data.get("html_url", f"https://github.com/{REPO}/releases")))
            self.banners.addWidget(Banner(f"Depthpad {tag} is available.", action=btn))

    # --- profiles ----------------------------------------------------------------------------
    def _load_profile_into_ui(self):
        self._rebuild_strip()
        self.mapping.set_profile(self.profile)
        self.settings.set_profile(self.profile)
        self.bindings = self.mapping._bindings()

    def profile_changed(self):
        problems = validate(self.profile, self.key_names)
        if problems:
            log.warning("profile not applied: %s", problems)
            return
        self.engine.set_profile(copy.deepcopy(self.profile))
        self.bindings = self.mapping._bindings()
        self.save_timer.start()

    def switch_profile(self, name):
        if name == self.profile["name"]:
            return
        if self.save_timer.isActive():
            self.save_timer.stop()
            self.store.save(self.profile)
        self.profile = self.store.load(name)
        self.set_pref("profile", name)
        self.engine.set_profile(copy.deepcopy(self.profile))
        self._load_profile_into_ui()

    def _activate_new(self, p):
        self.store.save(p)
        self.profile = {"name": None}
        self.switch_profile(p["name"])

    def new_profile(self):
        p = copy.deepcopy(BUILTIN["Default"])
        p["name"] = self.store.unique_name("New profile")
        self._activate_new(p)

    def duplicate_profile(self):
        p = copy.deepcopy(self.profile)
        p["name"] = self.store.unique_name(self.profile["name"] + " copy")
        self._activate_new(p)

    def rename_profile(self):
        name, ok = QInputDialog.getText(self, "Rename profile", "Name", text=self.profile["name"])
        name = name.strip()
        if ok and name and name != self.profile["name"]:
            if name in self.store.names():
                QMessageBox.warning(self, APP_NAME, f"A profile named “{name}” already exists.")
                return
            old = self.profile["name"]
            p = copy.deepcopy(self.profile)
            p["name"] = name
            self.store.save(p)
            self.store.delete(old)
            self._activate_new(p)

    def delete_profile(self):
        names = self.store.names()
        if len(names) <= 1:
            QMessageBox.information(self, APP_NAME, "This is the only profile, so it can't be deleted.")
            return
        if QMessageBox.question(self, APP_NAME, f"Delete the profile “{self.profile['name']}”?") == QMessageBox.Yes:
            gone = self.profile["name"]
            self.store.delete(gone)
            self.profile = {"name": None}
            self.switch_profile([n for n in names if n != gone][0])

    def import_profile(self):
        path, _ = QFileDialog.getOpenFileName(self, "Import profile", "", "Depthpad profile (*.json)")
        if not path:
            return
        try:
            p = ProfileStore.load_file(path)
        except (OSError, ValueError, KeyError) as e:
            QMessageBox.warning(self, APP_NAME, f"Couldn't read that file: {e}")
            return
        problems = validate(p, self.key_names)
        if problems:
            QMessageBox.warning(self, APP_NAME, "That profile has problems:\n• " + "\n• ".join(problems[:8]))
            return
        p["name"] = self.store.unique_name(p["name"])
        self._activate_new(p)

    def export_profile(self):
        path, _ = QFileDialog.getSaveFileName(self, "Export profile", self.profile["name"] + ".json",
                                              "Depthpad profile (*.json)")
        if path:
            with open(path, "w", encoding="utf8") as f:
                json.dump(self.profile, f, indent=2)

    def _replace_bindings(self, bindings, message):
        before = copy.deepcopy(self.profile["bindings"])
        self.profile["bindings"] = bindings
        self.mapping.set_profile(self.profile)
        self.profile_changed()
        undo = QPushButton("Undo")
        banner = Banner(message, action=undo)

        def restore():
            self.profile["bindings"] = before
            self.mapping.set_profile(self.profile)
            self.profile_changed()
            banner.deleteLater()

        undo.clicked.connect(restore)
        self.banners.addWidget(banner)
        QTimer.singleShot(9000, lambda: banner.deleteLater() if banner.isVisible() else None)

    def clear_mappings(self):
        if not self.profile["bindings"]:
            return
        if QMessageBox.question(self, APP_NAME, f"Remove every mapping from “{self.profile['name']}”?") == QMessageBox.Yes:
            self._replace_bindings([], f"All mappings removed from “{self.profile['name']}”.")

    def reset_mappings(self):
        base = BUILTIN.get(self.profile["name"], BUILTIN["Default"])
        if QMessageBox.question(self, APP_NAME, f"Replace the mappings in “{self.profile['name']}” with the "
                                f"{base['name']} layout?") == QMessageBox.Yes:
            self._replace_bindings(copy.deepcopy(base["bindings"]),
                                   f"Mappings reset to the {base['name']} layout.")

    def set_pref(self, key, value):
        self.prefs[key] = value
        self.prefs.save()

    # --- diagnostics -------------------------------------------------------------------------
    def toggle_recording(self):
        """Restart the engine with or without a session recording."""
        self.engine.stop()
        if self.recording:
            path, self.recording = self.recording, None
            self.about.record.setText("Start recording")
            self.about.record.setObjectName("")
            QMessageBox.information(self, APP_NAME, f"Recording saved:\n{path}")
            self.engine = Engine(copy.deepcopy(self.profile))
        else:
            os.makedirs(appdata.LOG_DIR, exist_ok=True)
            self.recording = os.path.join(appdata.LOG_DIR, time.strftime("session-%Y%m%d-%H%M%S.log"))
            self.about.record.setText("Stop recording")
            self.about.record.setObjectName("Primary")
            self.engine = Engine(copy.deepcopy(self.profile), log_path=self.recording)
        self.about.record.style().unpolish(self.about.record)
        self.about.record.style().polish(self.about.record)
        self.engine.start()

    # --- live updates ------------------------------------------------------------------------
    def _tick(self):
        s = self.engine.snapshot()
        state = s["state"]
        if state == "streaming":
            self.kb_status.set(f"AK680 V2 · {s['rate']:.0f} depth updates/s", T.OK)
        elif state == "searching":
            self.kb_status.set(s["error"] or "Keyboard not found", T.ERROR)
        else:
            self.kb_status.set("Connecting to the keyboard…", T.WARN)
        if s["paused"]:
            self.pad_status.set("Controller paused", T.WARN)
        elif s["pad"]:
            self.pad_status.set("Controller active" + (" · keys blocked" if s["blocking"] else ""), T.OK)
        else:
            self.pad_status.set("No virtual controller", T.ERROR)
        paused = s["paused"]
        if self.pause_btn.icon_name != ("play" if paused else "pause"):
            self.pause_btn.set_icon("play" if paused else "pause", accent=paused)
            self.pause_btn.setToolTip("Resume the controller" if paused else "Pause the controller")
            self.tray_pause.setText("Resume" if paused else "Pause")
        if s["contention"] and not self.contention_banner:
            self.contention_banner = Banner("Another app keeps switching the keyboard's analog mode off. Close the "
                                            "AJAZZ web driver or other keyboard software while using Depthpad.")
            self.banners.addWidget(self.contention_banner)
        elif not s["contention"] and self.contention_banner:
            self.contention_banner.deleteLater()
            self.contention_banner = None
        self.live.analysis.push(time.perf_counter() - self.t0, s["output"])
        page = self.stack.currentWidget()
        if page is self.live:
            self.live.update_state(s, self.bindings)
        elif page is self.mapping:
            self.mapping.update_state(s)

    # --- window lifecycle --------------------------------------------------------------------
    def showEvent(self, e):
        super().showEvent(e)
        T.dark_title_bar(self)

    def show_window(self):
        self.showNormal()
        self.raise_()
        self.activateWindow()

    def closeEvent(self, e):
        if self.prefs["close_to_tray"] and not self.quitting and QSystemTrayIcon.isSystemTrayAvailable():
            e.ignore()
            self.hide()
            if not self.prefs.get("tray_hint_shown"):
                self.tray.showMessage(APP_NAME, "Depthpad keeps running in the tray. Right-click the icon to quit.",
                                      app_icon(), 4000)
                self.set_pref("tray_hint_shown", True)
            return
        self.quit()

    def quit(self):
        self.quitting = True
        self.tick.stop()
        if self.save_timer.isActive():
            self.store.save(self.profile)
        self.engine.stop()
        self.tray.hide()
        QApplication.quit()


def self_test(path):
    """Headless check of a build: engine, keyboard stream and virtual controller. Writes JSON to path."""
    result = {"version": VERSION, "frozen": bool(getattr(sys, "frozen", False)), "vigembus": appdata.vigembus_installed()}
    try:
        load_keymap()
        engine = Engine(copy.deepcopy(BUILTIN["Default"]))
        engine.start()
        time.sleep(2.5)
        engine.snapshot()
        time.sleep(0.5)
        s = engine.snapshot()
        result.update(state=s["state"], rate=round(s["rate"]), firmware=s["firmware"], pad=s["pad"],
                      pad_error=s["pad_error"], events=[e[1] for e in engine.events])
        engine.stop()
        result["ok"] = True
    except Exception as e:  # report everything; this is a diagnostic
        result.update(ok=False, error=repr(e))
    with open(path, "w", encoding="utf8") as f:
        json.dump(result, f, indent=2)
    return 0 if result["ok"] else 1


def apply_style(app):
    app.setStyle("Fusion")
    app.setPalette(T.palette())
    app.setFont(T.font(T.BODY))
    cache = os.path.join(appdata.DATA_DIR, "cache")
    os.makedirs(cache, exist_ok=True)
    app.setStyleSheet(T.stylesheet(chevron_png(os.path.join(cache, "chevron.png"))))
    app.setWindowIcon(app_icon())


def main():
    if "--self-test" in sys.argv:
        i = sys.argv.index("--self-test")
        return self_test(sys.argv[i + 1] if i + 1 < len(sys.argv) else "depthpad-selftest.json")
    minimized = "--minimized" in sys.argv
    QApplication.setHighDpiScaleFactorRoundingPolicy(Qt.HighDpiScaleFactorRoundingPolicy.PassThrough)
    app = QApplication(sys.argv)
    app.setApplicationName(APP_NAME)
    app.setApplicationVersion(VERSION)
    app.setQuitOnLastWindowClosed(False)

    sock = QLocalSocket()
    sock.connectToServer(INSTANCE_KEY)
    if sock.waitForConnected(300):
        sock.write(b"show")
        sock.waitForBytesWritten(300)
        return 0
    server = QLocalServer()
    QLocalServer.removeServer(INSTANCE_KEY)
    server.listen(INSTANCE_KEY)

    try:
        import ctypes
        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("Depthpad.App")
    except (AttributeError, OSError):
        pass
    appdata.setup_logging()
    log.info("Depthpad %s starting", VERSION)
    apply_style(app)

    prefs = appdata.Prefs()
    win = MainWindow(prefs)
    server.newConnection.connect(lambda: (server.nextPendingConnection(), win.show_window()))
    if not (minimized or prefs["start_minimized"]):
        win.show()
    return app.exec()
