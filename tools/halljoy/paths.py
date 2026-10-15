"""Where the HallJoy checkout and this repository are. HALLJOY_DIR defaults to a HallJoy checkout
next to this repository; ANALOGSENSE_JS is AnalogSense.js from github.com/AnalogSense/JavaScript-SDK."""
import os

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
CORE = os.path.join(REPO, "crates", "depthpad-core")
HALLJOY = os.environ.get("HALLJOY_DIR", os.path.join(REPO, "..", "HallJoy"))
HALLJOY_SRC = os.path.join(HALLJOY, "src", "HallJoyProject", "HallJoy")
ANALOGSENSE_JS = os.environ.get("ANALOGSENSE_JS", os.path.join(REPO, "..", "AnalogSense.js"))
