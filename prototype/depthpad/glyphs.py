"""Gamepad glyphs: one recognisable mark per controller input, drawn at any size.

The same glyph is used on keys, on the input chips and as the drag preview, so a
mapping reads at a glance (as in HallJoy's remap view): face buttons in their
colours, bumpers and triggers as their shapes, stick directions as a stick cap
with an arrow, d-pad directions as a cross with the active arm lit.
"""
import math

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QFont, QPainter, QPainterPath, QPen, QPixmap

from . import theme as T

_c = T.color
NEUTRAL = "#C9CED6"      # light glyph ink on dark keys
NEUTRAL_FILL = "#3A3F47"


def _arrow(p, c, r, angle, color, width):
    """A chevron pointing at `angle` (radians, 0 = right, counter-clockwise)."""
    dx, dy = math.cos(angle), -math.sin(angle)
    tip = QPointF(c.x() + dx * r * 0.55, c.y() + dy * r * 0.55)
    back = QPointF(c.x() - dx * r * 0.05, c.y() - dy * r * 0.05)
    left = QPointF(back.x() - dy * r * 0.45, back.y() + dx * r * 0.45)
    right = QPointF(back.x() + dy * r * 0.45, back.y() - dx * r * 0.45)
    p.setPen(QPen(_c(color), width, Qt.SolidLine, Qt.RoundCap, Qt.RoundJoin))
    p.setBrush(Qt.NoBrush)
    p.drawPolyline([left, tip, right])


def draw(p, rect, target, active=False):
    """Draw the glyph for `target` centred in `rect` (square-ish)."""
    s = min(rect.width(), rect.height())
    c = rect.center()
    r = s / 2
    p.save()
    p.setRenderHint(QPainter.Antialiasing)
    if target in ("A", "B", "X", "Y"):
        col = T.FACE[target]
        p.setPen(QPen(_c(col).darker(135), max(1.0, s * 0.06)))
        p.setBrush(_c(col))
        p.drawEllipse(c, r * 0.92, r * 0.92)
        p.setPen(_c("#FFFFFF"))
        p.setFont(T.font(max(5.5, s * 0.36), QFont.Black))
        p.drawText(rect, Qt.AlignCenter, target)
    elif target in ("LB", "RB"):
        body = QRectF(c.x() - r * 0.95, c.y() - r * 0.42, r * 1.9, r * 0.84)
        p.setPen(QPen(_c(NEUTRAL), max(1.0, s * 0.06)))
        p.setBrush(_c(T.ACCENT if active else NEUTRAL_FILL))
        p.drawRoundedRect(body, r * 0.42, r * 0.42)
        p.setPen(_c("#FFFFFF"))
        p.setFont(T.font(max(5.0, s * 0.27), QFont.Bold))
        p.drawText(body, Qt.AlignCenter, target)
    elif target in ("LT", "RT"):
        path = QPainterPath()
        left, right = c.x() - r * 0.62, c.x() + r * 0.62
        top, bottom = c.y() - r * 0.9, c.y() + r * 0.9
        path.moveTo(left, bottom)
        path.lineTo(left, top + r * 0.55)
        path.quadTo(left, top, c.x(), top)
        path.quadTo(right, top, right, top + r * 0.55)
        path.lineTo(right, bottom)
        path.closeSubpath()
        p.setPen(QPen(_c(NEUTRAL), max(1.0, s * 0.06)))
        p.setBrush(_c(T.ACCENT if active else NEUTRAL_FILL))
        p.drawPath(path)
        p.setPen(_c("#FFFFFF"))
        p.setFont(T.font(max(5.0, s * 0.27), QFont.Bold))
        p.drawText(QRectF(left, c.y() - r * 0.1, right - left, r * 0.9), Qt.AlignCenter, target)
    elif target in ("LS", "RS") or target.startswith(("LS_", "RS_")):
        side = target[0]
        p.setPen(QPen(_c(NEUTRAL), max(1.0, s * 0.06)))
        p.setBrush(_c(NEUTRAL_FILL))
        p.drawEllipse(c, r * 0.92, r * 0.92)
        p.setBrush(_c(T.ACCENT if active else "#4A5059"))
        p.setPen(Qt.NoPen)
        p.drawEllipse(c, r * 0.62, r * 0.62)
        if "_" in target:
            angle = {"UP": math.pi / 2, "DOWN": -math.pi / 2, "LEFT": math.pi, "RIGHT": 0}[target.split("_")[1]]
            _arrow(p, c, r, angle, "#FFFFFF", max(1.2, s * 0.09))
            # which stick: a small L / R tab on the corner
            tab = QRectF(rect.right() - s * 0.36, rect.bottom() - s * 0.36, s * 0.36, s * 0.36)
            p.setPen(Qt.NoPen)
            p.setBrush(_c("#FFFFFF"))
            p.drawEllipse(tab)
            p.setPen(_c("#15181D"))
            p.setFont(T.font(max(4.5, s * 0.22), QFont.Black))
            p.drawText(tab, Qt.AlignCenter, side)
        else:
            p.setPen(_c("#FFFFFF"))
            p.setFont(T.font(max(5.0, s * 0.27), QFont.Black))
            p.drawText(rect, Qt.AlignCenter, f"{side}3")
    elif target.startswith("DPAD_"):
        arm = r * 0.34
        cross = QPainterPath()
        cross.addRoundedRect(QRectF(c.x() - arm, c.y() - r * 0.92, arm * 2, r * 1.84), arm * 0.4, arm * 0.4)
        cross.addRoundedRect(QRectF(c.x() - r * 0.92, c.y() - arm, r * 1.84, arm * 2), arm * 0.4, arm * 0.4)
        cross = cross.simplified()
        p.setPen(QPen(_c(NEUTRAL), max(1.0, s * 0.06)))
        p.setBrush(_c(NEUTRAL_FILL))
        p.drawPath(cross)
        d = target.split("_")[1]
        dx, dy = {"UP": (0, -1), "DOWN": (0, 1), "LEFT": (-1, 0), "RIGHT": (1, 0)}[d]
        lit = QRectF(c.x() + dx * r * 0.58 - arm * 0.75, c.y() + dy * r * 0.58 - arm * 0.75, arm * 1.5, arm * 1.5)
        p.setPen(Qt.NoPen)
        p.setBrush(_c(T.ACCENT if active else "#FFFFFF"))
        p.drawRoundedRect(lit, arm * 0.3, arm * 0.3)
    elif target in ("BACK", "START", "GUIDE"):
        p.setPen(QPen(_c(NEUTRAL), max(1.0, s * 0.06)))
        p.setBrush(_c(T.ACCENT if active else NEUTRAL_FILL))
        p.drawEllipse(c, r * 0.92, r * 0.92)
        p.setPen(QPen(_c("#FFFFFF"), max(1.0, s * 0.07), Qt.SolidLine, Qt.RoundCap, Qt.RoundJoin))
        p.setBrush(Qt.NoBrush)
        k = r * 0.32
        if target == "BACK":        # two overlapping windows
            p.drawRoundedRect(QRectF(c.x() - k * 1.1, c.y() - k * 0.9, k * 1.4, k * 1.2), 1.5, 1.5)
            p.drawRoundedRect(QRectF(c.x() - k * 0.3, c.y() - k * 0.3, k * 1.4, k * 1.2), 1.5, 1.5)
        elif target == "START":     # three lines
            for i in (-1, 0, 1):
                p.drawLine(QPointF(c.x() - k, c.y() + i * k * 0.7), QPointF(c.x() + k, c.y() + i * k * 0.7))
        else:                       # guide: a ring
            p.drawEllipse(c, k, k)
    p.restore()


def pixmap(target, size, active=False, dpr=2.0):
    pm = QPixmap(int(size * dpr), int(size * dpr))
    pm.setDevicePixelRatio(dpr)
    pm.fill(Qt.transparent)
    p = QPainter(pm)
    draw(p, QRectF(1, 1, size - 2, size - 2), target, active)
    p.end()
    return pm
