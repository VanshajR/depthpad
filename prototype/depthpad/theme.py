"""Design system: tokens, type scale and the application stylesheet.

Principles (see docs/DESIGN.md):
  * neutral charcoal surfaces, separated by tone; hairline borders only on raised panels
  * one accent (blue) for selection and measured depth; amber only for "held"; status colours only for status
  * 8 px grid, 12 px panel radius, 36 px controls
  * text in three inks (primary / secondary / tertiary); data never colours text
"""
from PySide6.QtGui import QColor, QFont, QPalette

# Neutral charcoal ramp (no hue tint).
BG = "#101010"           # window plane
RAIL = "#141414"         # navigation rail
SURFACE = "#1A1A1A"      # panels
RAISED = "#212121"       # controls, chips, keycaps
HOVER = "#2A2A2A"
PRESSED = "#323232"
STROKE = "#2B2B2B"       # hairline borders on raised panels
STROKE_SOFT = "#232323"  # dividers, gridlines
SHADOW = "#000000"

KEY = "#262626"
KEY_EDGE = "#1E1E1E"
PLATE = "#161616"

INK = "#F2F2F2"          # primary text
INK_2 = "#A6A6A6"        # secondary text
INK_3 = "#6E6E6E"        # tertiary text, axis labels

ACCENT = "#4C8DFF"
ACCENT_HOVER = "#62A0FF"
ACCENT_INK = "#9CC0FF"   # accent-tinted text on dark (labels on mapped keys)
ACCENT_WASH = "rgba(76, 141, 255, 0.14)"
HELD = "#F0A33A"
OK = "#2FBF71"
WARN = "#F0A33A"
ERROR = "#EF5350"

# Chart series (dataviz reference palette, dark steps, fixed order).
SERIES = ["#3987E5", "#D95926", "#199E70", "#C98500"]
FACE = {"A": "#2FBF71", "B": "#EF5350", "X": "#4C8DFF", "Y": "#E6B422"}

FONT_TEXT = "Segoe UI Variable Text"
FONT_DISPLAY = "Segoe UI Variable Display"
FONT_FALLBACK = "Segoe UI"

# Type scale (pt): title, heading, body, label, caption, overline.
TITLE, HEADING, BODY, LABEL, CAPTION, OVERLINE = 17.0, 11.5, 10.0, 9.5, 9.0, 8.0


def color(hex_, alpha=None):
    c = QColor(hex_)
    if alpha is not None:
        c.setAlphaF(alpha)
    return c


def font(size=BODY, weight=QFont.Normal, display=False, tabular=False, spacing=None):
    f = QFont(FONT_DISPLAY if display else FONT_TEXT)
    f.setFamilies([f.family(), FONT_FALLBACK])
    f.setPointSizeF(size)
    f.setWeight(weight)
    f.setHintingPreference(QFont.PreferNoHinting)
    if tabular:
        f.setFeature(QFont.Tag("tnum"), 1)
    if spacing is not None:
        f.setLetterSpacing(QFont.AbsoluteSpacing, spacing)
    return f


def palette():
    p = QPalette()
    for role, c in ((QPalette.Window, BG), (QPalette.WindowText, INK), (QPalette.Base, RAISED),
                    (QPalette.AlternateBase, SURFACE), (QPalette.Text, INK), (QPalette.Button, RAISED),
                    (QPalette.ButtonText, INK), (QPalette.Highlight, ACCENT), (QPalette.HighlightedText, "#FFFFFF"),
                    (QPalette.ToolTipBase, RAISED), (QPalette.ToolTipText, INK), (QPalette.PlaceholderText, INK_3),
                    (QPalette.Light, HOVER), (QPalette.Mid, STROKE), (QPalette.Dark, BG)):
        p.setColor(role, QColor(c))
    return p


STYLESHEET = f"""
* {{ outline: none; }}
QWidget {{ color: {INK}; font-size: {BODY}pt; }}
#Root, #Plane {{ background: {BG}; }}
QToolTip {{ background: {RAISED}; color: {INK}; border: 1px solid {STROKE}; border-radius: 8px; padding: 7px 10px; }}

#Rail {{ background: {RAIL}; border-right: 1px solid {STROKE_SOFT}; }}

#Panel {{ background: {SURFACE}; border: 1px solid {STROKE_SOFT}; border-radius: 12px; }}
#Group {{ background: {SURFACE}; border: 1px solid {STROKE_SOFT}; border-radius: 12px; }}
#Divider {{ background: {STROKE_SOFT}; max-height: 1px; min-height: 1px; }}

#Title {{ font-size: {TITLE}pt; font-weight: 600; }}
#Heading {{ font-size: {HEADING}pt; font-weight: 600; }}
#Overline {{ color: {INK_3}; font-size: {OVERLINE}pt; font-weight: 700; letter-spacing: 0.8px; }}
#Secondary {{ color: {INK_2}; }}
#Tertiary {{ color: {INK_3}; font-size: {CAPTION}pt; }}
#Strong {{ font-weight: 600; }}
#Metric {{ font-size: 16pt; font-weight: 600; }}
#Unit {{ color: {INK_3}; font-size: {CAPTION}pt; }}

QPushButton {{ background: {RAISED}; border: 1px solid {STROKE}; border-radius: 8px; padding: 0 14px; min-height: 34px; }}
QPushButton:hover {{ background: {HOVER}; }}
QPushButton:pressed {{ background: {PRESSED}; }}
QPushButton:disabled {{ color: {INK_3}; }}
QPushButton#Primary {{ background: {ACCENT}; border-color: {ACCENT}; color: white; font-weight: 600; }}
QPushButton#Primary:hover {{ background: {ACCENT_HOVER}; border-color: {ACCENT_HOVER}; }}
QPushButton#Quiet {{ background: transparent; border-color: transparent; color: {INK_2}; }}
QPushButton#Quiet:hover {{ background: {RAISED}; color: {INK}; }}
QPushButton#Danger {{ background: transparent; border-color: transparent; color: {ERROR}; }}
QPushButton#Danger:hover {{ background: rgba(239, 83, 80, 0.12); }}
QPushButton#Chip {{ background: {RAISED}; border: 1px solid {STROKE}; border-radius: 7px; padding: 0 10px;
                    min-height: 30px; min-width: 44px; color: {INK_2}; font-size: {LABEL}pt; font-weight: 600; }}
QPushButton#Chip:hover {{ color: {INK}; background: {HOVER}; }}
QPushButton#Chip:checked {{ background: {ACCENT_WASH}; border-color: {ACCENT}; color: {INK}; }}
QPushButton#Chip:disabled {{ color: {INK_3}; background: transparent; }}
QPushButton#Chip[hint="true"] {{ border-color: {ACCENT_INK}; color: {INK}; }}
QPushButton#Chip[dim="true"] {{ color: {INK_3}; }}

QComboBox {{ background: {RAISED}; border: 1px solid {STROKE}; border-radius: 8px; padding: 0 12px; min-height: 34px; }}
QComboBox:hover {{ background: {HOVER}; }}
QComboBox::drop-down {{ border: none; width: 26px; }}
QComboBox QAbstractItemView {{ background: {RAISED}; border: 1px solid {STROKE}; border-radius: 8px; padding: 4px;
                               selection-background-color: {HOVER}; selection-color: {INK}; }}
QLineEdit, QDoubleSpinBox {{ background: {BG}; border: 1px solid {STROKE}; border-radius: 7px; padding: 0 8px;
                             min-height: 30px; selection-background-color: {ACCENT}; }}
QLineEdit:focus, QDoubleSpinBox:focus {{ border-color: {ACCENT}; }}
QDoubleSpinBox::up-button, QDoubleSpinBox::down-button {{ width: 0; border: none; }}

QSlider {{ min-height: 22px; }}
QSlider::groove:horizontal {{ height: 4px; background: {PRESSED}; border-radius: 2px; }}
QSlider::sub-page:horizontal {{ background: {ACCENT}; border-radius: 2px; }}
QSlider::handle:horizontal {{ background: {INK}; width: 16px; height: 16px; margin: -6px 0; border-radius: 8px; }}
QSlider::handle:horizontal:hover {{ background: white; }}
QSlider::handle:horizontal:disabled {{ background: {INK_3}; }}
QSlider::sub-page:horizontal:disabled {{ background: {PRESSED}; }}

QScrollArea {{ background: transparent; border: none; }}
QScrollArea > QWidget > QWidget {{ background: transparent; }}
QScrollBar:vertical {{ background: transparent; width: 10px; margin: 4px 2px; }}
QScrollBar::handle:vertical {{ background: {PRESSED}; border-radius: 3px; min-height: 30px; }}
QScrollBar::handle:vertical:hover {{ background: #3D3D3D; }}
QScrollBar:horizontal {{ background: transparent; height: 8px; margin: 0 4px; }}
QScrollBar::handle:horizontal {{ background: {PRESSED}; border-radius: 3px; min-width: 30px; }}
QScrollBar::add-line, QScrollBar::sub-line {{ height: 0; width: 0; }}
QScrollBar::add-page, QScrollBar::sub-page {{ background: transparent; }}

QMenu {{ background: {RAISED}; border: 1px solid {STROKE}; border-radius: 10px; padding: 5px; }}
QMenu::item {{ padding: 7px 22px 7px 12px; border-radius: 6px; }}
QMenu::item:selected {{ background: {HOVER}; }}
QMenu::separator {{ height: 1px; background: {STROKE}; margin: 4px 6px; }}

QMessageBox, QInputDialog, QDialog {{ background: {SURFACE}; }}

#Banner {{ background: rgba(240, 163, 58, 0.08); border: 1px solid rgba(240, 163, 58, 0.25); border-radius: 10px; }}
#BannerError {{ background: rgba(239, 83, 80, 0.08); border: 1px solid rgba(239, 83, 80, 0.25); border-radius: 10px; }}
"""


def stylesheet(chevron_path=None):
    """The stylesheet, with a drawn chevron for drop-downs (QSS can only load images from files)."""
    css = STYLESHEET
    if chevron_path:
        p = chevron_path.replace("\\", "/")
        css += f"""
QComboBox::down-arrow {{ image: url("{p}"); width: 10px; height: 10px; }}
QComboBox::drop-down {{ subcontrol-position: center right; right: 10px; }}
"""
    return css


def dark_title_bar(widget, caption=RAIL):
    """Windows 10/11: dark title bar; Windows 11: caption and border coloured to match the app."""
    import ctypes
    try:
        hwnd = int(widget.winId())
        dwm = ctypes.windll.dwmapi
        on = ctypes.c_int(1)
        dwm.DwmSetWindowAttribute(hwnd, 20, ctypes.byref(on), 4)          # DWMWA_USE_IMMERSIVE_DARK_MODE
        c = QColor(caption)
        ref = ctypes.c_int(c.red() | (c.green() << 8) | (c.blue() << 16))
        dwm.DwmSetWindowAttribute(hwnd, 35, ctypes.byref(ref), 4)         # DWMWA_CAPTION_COLOR
        dwm.DwmSetWindowAttribute(hwnd, 34, ctypes.byref(ref), 4)         # DWMWA_BORDER_COLOR
    except (AttributeError, OSError):
        pass
