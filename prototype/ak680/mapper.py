"""Profile + key depths -> controller report."""
import math

from .profile import (STICK_TARGETS, TRIGGER_TARGETS, BUTTON_TARGETS, apply_curve, curve_of, MAX_TRAVEL_MM)
from .sticks import AxisResolver
from .vigem import BUTTONS


class Mapper:
    def __init__(self, profile, keymap):
        by_name = {v["key"]: pos for pos, v in keymap.items()}
        self.profile = profile
        s = profile["settings"]
        self.circle = s.get("stick_shape", "square") == "circle"
        self.targets = {}           # target -> [(pos, curve)]
        for b in profile["bindings"]:
            pos = by_name.get(b["key"])
            if pos is not None:
                self.targets.setdefault(b["target"], []).append((pos, curve_of(profile, b)))
        self.positions = {p for lst in self.targets.values() for p, _ in lst}
        hk = s.get("pause_hotkey") or ""
        self.hotkey = [by_name[n] for n in hk.split("+") if n in by_name] if hk else []
        new = lambda: AxisResolver(s.get("snappy_joystick", False), s.get("last_key_priority", False),
                                   s.get("last_key_priority_sensitivity", 0.02))
        self.resolvers = {("LS", "x"): new(), ("LS", "y"): new(), ("RS", "x"): new(), ("RS", "y"): new()}

    def value(self, fusion, target, now):
        """Deepest curved value among the keys bound to a target (HallJoy's ReadSide)."""
        best = 0.0
        for pos, curve in self.targets.get(target, ()):
            best = max(best, apply_curve(fusion.depth(pos, now), curve))
        return best

    def pressed(self, fusion, target, now):
        return any(fusion.depth(pos, now) * MAX_TRAVEL_MM >= curve["press_mm"]
                   for pos, curve in self.targets.get(target, ()))

    def compute(self, fusion, now):
        axes = []
        for stick in ("LS", "RS"):
            v = {d: self.value(fusion, f"{stick}_{d}", now) for d in ("UP", "DOWN", "LEFT", "RIGHT")}
            x = self.resolvers[(stick, "x")].resolve(v["LEFT"], v["RIGHT"])
            y = self.resolvers[(stick, "y")].resolve(v["DOWN"], v["UP"])
            m = math.hypot(x, y)
            if self.circle and m > 1:
                x, y = x / m, y / m
            axes += [x, y]
        lt, rt = self.value(fusion, "LT", now), self.value(fusion, "RT", now)
        buttons = 0
        for target in BUTTON_TARGETS:
            if target in self.targets and self.pressed(fusion, target, now):
                buttons |= BUTTONS[target]
        return buttons, lt, rt, axes
