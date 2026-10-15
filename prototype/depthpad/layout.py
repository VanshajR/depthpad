"""AK680 V2 geometry for drawing: (position, legend, width in key units) per row,
plus short labels for controller targets."""

ROWS = [
    [(0, "Esc", 1), (17, "1", 1), (18, "2", 1), (19, "3", 1), (20, "4", 1), (21, "5", 1), (22, "6", 1),
     (23, "7", 1), (24, "8", 1), (25, "9", 1), (26, "0", 1), (27, "-", 1), (28, "=", 1), (92, "Backspace", 2),
     (104, "Home", 1)],
    [(32, "Tab", 1.5), (33, "Q", 1), (34, "W", 1), (35, "E", 1), (36, "R", 1), (37, "T", 1), (38, "Y", 1),
     (39, "U", 1), (40, "I", 1), (41, "O", 1), (42, "P", 1), (43, "[", 1), (44, "]", 1), (60, "\\", 1.5),
     (106, "Del", 1)],
    [(48, "Caps", 1.75), (49, "A", 1), (50, "S", 1), (51, "D", 1), (52, "F", 1), (53, "G", 1), (54, "H", 1),
     (55, "J", 1), (56, "K", 1), (57, "L", 1), (58, ";", 1), (59, "'", 1), (76, "Enter", 2.25), (105, "PgUp", 1)],
    [(64, "Shift", 2.25), (65, "Z", 1), (66, "X", 1), (67, "C", 1), (68, "V", 1), (69, "B", 1), (70, "N", 1),
     (71, "M", 1), (72, ",", 1), (73, ".", 1), (74, "/", 1), (75, "Shift", 1.75), (90, "↑", 1), (108, "PgDn", 1)],
    [(80, "Ctrl", 1.25), (81, "Win", 1.25), (82, "Alt", 1.25), (83, "", 6.25), (84, "Alt", 1), (85, "Fn", 1),
     (87, "Ctrl", 1), (88, "←", 1), (89, "↓", 1), (91, "→", 1)],
]
WIDTH_UNITS = 16
assert all(abs(sum(w for _, _, w in row) - WIDTH_UNITS) < 1e-9 for row in ROWS)

LEGEND = {pos: legend for row in ROWS for pos, legend, _ in row}
LEGEND[83] = "Space"

TARGET_SHORT = {
    "LS_UP": "LS ↑", "LS_DOWN": "LS ↓", "LS_LEFT": "LS ←", "LS_RIGHT": "LS →",
    "RS_UP": "RS ↑", "RS_DOWN": "RS ↓", "RS_LEFT": "RS ←", "RS_RIGHT": "RS →",
    "LT": "LT", "RT": "RT", "A": "A", "B": "B", "X": "X", "Y": "Y", "LB": "LB", "RB": "RB",
    "BACK": "View", "START": "Menu", "LS": "L3", "RS": "R3",
    "DPAD_UP": "D ↑", "DPAD_DOWN": "D ↓", "DPAD_LEFT": "D ←", "DPAD_RIGHT": "D →", "GUIDE": "Guide",
}

TARGET_GROUPS = [
    ("Left stick", ["LS_UP", "LS_DOWN", "LS_LEFT", "LS_RIGHT"]),
    ("Right stick", ["RS_UP", "RS_DOWN", "RS_LEFT", "RS_RIGHT"]),
    ("Triggers", ["LT", "RT"]),
    ("Face", ["A", "B", "X", "Y"]),
    ("Bumpers", ["LB", "RB"]),
    ("System", ["BACK", "START", "GUIDE"]),
    ("Clicks", ["LS", "RS"]),
    ("D-pad", ["DPAD_UP", "DPAD_DOWN", "DPAD_LEFT", "DPAD_RIGHT"]),
]

TARGETS_LONG = {
    "LS_UP": "Left stick up", "LS_DOWN": "Left stick down", "LS_LEFT": "Left stick left", "LS_RIGHT": "Left stick right",
    "RS_UP": "Right stick up", "RS_DOWN": "Right stick down", "RS_LEFT": "Right stick left", "RS_RIGHT": "Right stick right",
    "LT": "Left trigger", "RT": "Right trigger", "A": "A button", "B": "B button", "X": "X button", "Y": "Y button",
    "LB": "Left bumper", "RB": "Right bumper", "BACK": "View button", "START": "Menu button", "GUIDE": "Guide button",
    "LS": "Left stick click", "RS": "Right stick click", "DPAD_UP": "D-pad up", "DPAD_DOWN": "D-pad down",
    "DPAD_LEFT": "D-pad left", "DPAD_RIGHT": "D-pad right",
}
