"""Where things live, app preferences, start-with-Windows and ViGEmBus detection."""
import json
import logging
import logging.handlers
import os
import sys
import winreg

from . import APP_ID

CONFIG_DIR = os.path.join(os.environ.get("APPDATA", os.path.expanduser("~")), APP_ID)
DATA_DIR = os.path.join(os.environ.get("LOCALAPPDATA", os.path.expanduser("~")), APP_ID)
PROFILE_DIR = os.path.join(CONFIG_DIR, "profiles")
LOG_DIR = os.path.join(DATA_DIR, "logs")
PREFS_FILE = os.path.join(CONFIG_DIR, "settings.json")
RUN_KEY = r"Software\Microsoft\Windows\CurrentVersion\Run"

PREFS_DEFAULTS = {
    "profile": "Default",
    "close_to_tray": True,
    "start_minimized": False,
    "check_updates": True,
    "seen_welcome": False,
}


def setup_logging():
    os.makedirs(LOG_DIR, exist_ok=True)
    h = logging.handlers.RotatingFileHandler(os.path.join(LOG_DIR, "depthpad.log"), maxBytes=1_000_000,
                                             backupCount=3, encoding="utf8")
    h.setFormatter(logging.Formatter("%(asctime)s %(levelname)s %(message)s"))
    root = logging.getLogger()
    root.setLevel(logging.INFO)
    root.addHandler(h)


class Prefs(dict):
    def __init__(self):
        super().__init__(PREFS_DEFAULTS)
        try:
            with open(PREFS_FILE, encoding="utf8") as f:
                self.update(json.load(f))
        except (OSError, ValueError):
            pass

    def save(self):
        os.makedirs(CONFIG_DIR, exist_ok=True)
        tmp = PREFS_FILE + ".tmp"
        with open(tmp, "w", encoding="utf8") as f:
            json.dump(dict(self), f, indent=2)
        os.replace(tmp, PREFS_FILE)


def _launch_command():
    if getattr(sys, "frozen", False):
        return f'"{sys.executable}" --minimized'
    pyw = os.path.join(os.path.dirname(sys.executable), "pythonw.exe")
    main = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "run_depthpad.py"))
    return f'"{pyw}" "{main}" --minimized'


def autostart_enabled():
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, RUN_KEY) as k:
            winreg.QueryValueEx(k, APP_ID)
            return True
    except OSError:
        return False


def set_autostart(enabled):
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, RUN_KEY, 0, winreg.KEY_SET_VALUE) as k:
        if enabled:
            winreg.SetValueEx(k, APP_ID, 0, winreg.REG_SZ, _launch_command())
        else:
            try:
                winreg.DeleteValue(k, APP_ID)
            except FileNotFoundError:
                pass


def vigembus_installed():
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Services\ViGEmBus"):
            return True
    except OSError:
        return False
