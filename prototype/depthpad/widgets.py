"""Depthpad component library (custom-drawn where Qt's stock widgets fall short)."""
import collections

from PySide6.QtCore import QMimeData, QPointF, QRectF, QSize, Qt, Signal, QVariantAnimation
from PySide6.QtGui import (QColor, QDrag, QFont, QLinearGradient, QPainter, QPainterPath, QPen, QPixmap,
                           QRadialGradient)
from PySide6.QtWidgets import (QButtonGroup, QDoubleSpinBox, QFrame, QGraphicsDropShadowEffect, QGridLayout,
                               QHBoxLayout, QLabel, QPushButton, QSizePolicy, QSlider, QToolButton, QVBoxLayout,
                               QWidget)

from . import icons
from . import theme as T
from .layout import ROWS, TARGET_GROUPS, TARGET_SHORT, WIDTH_UNITS

_c = T.color
MIME = "application/x-depthpad-target"   # payload: "<target>" or "<target>|<source key position>"


def start_drag(source, target, from_pos=None):
    """Drag a controller input (optionally moving it off a key) with a chip-shaped preview."""
    md = QMimeData()
    md.setData(MIME, (f"{target}|{from_pos}" if from_pos is not None else target).encode())
    from PySide6.QtCore import QPoint
    from PySide6.QtGui import QFontMetrics
    from . import glyphs
    label = TARGET_SHORT.get(target, target)
    f = T.font(T.LABEL, QFont.Bold)
    w = QFontMetrics(f).horizontalAdvance(label) + 58
    h = 38
    pm = QPixmap(int(w * 2), h * 2)
    pm.setDevicePixelRatio(2)
    pm.fill(Qt.transparent)
    p = QPainter(pm)
    p.setRenderHint(QPainter.Antialiasing)
    p.setPen(QPen(_c(T.ACCENT), 1.5))
    p.setBrush(_c(T.RAISED, 0.96))
    p.drawRoundedRect(QRectF(1, 1, w - 2, h - 2), 10, 10)
    glyphs.draw(p, QRectF(8, 7, 24, 24), target)
    p.setPen(_c(T.INK))
    p.setFont(f)
    p.drawText(QRectF(40, 0, w - 46, h), Qt.AlignVCenter | Qt.AlignLeft, label)
    p.end()
    drag = QDrag(source)
    drag.setMimeData(md)
    drag.setPixmap(pm)
    drag.setHotSpot(QPoint(20, h // 2))
    return drag.exec(Qt.MoveAction | Qt.CopyAction)


def text(s, role=None, wrap=False):
    w = QLabel(s)
    if role:
        w.setObjectName(role)
    w.setWordWrap(wrap)
    return w


def shadow(widget, blur=36, y=8, alpha=0.45):
    e = QGraphicsDropShadowEffect(widget)
    e.setBlurRadius(blur)
    e.setOffset(0, y)
    e.setColor(_c(T.SHADOW, alpha))
    widget.setGraphicsEffect(e)


# ------------------------------------------------------------------------------------------- containers
class Panel(QFrame):
    """A raised surface. Optional overline caption and a right-aligned header widget."""

    def __init__(self, caption=None, right=None, margins=(20, 18, 20, 20), spacing=14, parent=None):
        super().__init__(parent)
        self.setObjectName("Panel")
        self.body = QVBoxLayout(self)
        self.body.setContentsMargins(*margins)
        self.body.setSpacing(spacing)
        if caption or right:
            h = QHBoxLayout()
            h.setSpacing(10)
            if caption:
                h.addWidget(text(caption.upper(), "Overline"))
            h.addStretch(1)
            if right:
                h.addWidget(right)
            self.body.addLayout(h)


class Group(QFrame):
    """A settings group: rows separated by hairline dividers."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setObjectName("Group")
        self.lay = QVBoxLayout(self)
        self.lay.setContentsMargins(18, 2, 18, 2)
        self.lay.setSpacing(0)
        self._n = 0

    def add(self, widget):
        if self._n:
            d = QFrame()
            d.setObjectName("Divider")
            self.lay.addWidget(d)
        self.lay.addWidget(widget)
        self._n += 1
        return widget


class Row(QWidget):
    """Setting row: label and a one-line description on the left, the control on the right."""

    def __init__(self, title, desc=None, control=None, parent=None):
        super().__init__(parent)
        h = QHBoxLayout(self)
        h.setContentsMargins(0, 12, 0, 12)
        v = QVBoxLayout()
        v.setSpacing(2)
        v.addWidget(text(title))
        if desc:
            v.addWidget(text(desc, "Tertiary", wrap=True))
        h.addLayout(v, 1)
        if control is not None:
            h.addSpacing(32)
            h.addWidget(control, 0, Qt.AlignVCenter)


class Banner(QFrame):
    def __init__(self, message, error=False, action=None, parent=None):
        super().__init__(parent)
        self.setObjectName("BannerError" if error else "Banner")
        lay = QHBoxLayout(self)
        lay.setContentsMargins(14, 8, 8, 8)
        lay.setSpacing(10)
        ic = QLabel()
        ic.setPixmap(icons.pixmap("info", T.ERROR if error else T.WARN, 16))
        lay.addWidget(ic)
        lay.addWidget(text(message, wrap=True), 1)
        if action:
            lay.addWidget(action)


# ------------------------------------------------------------------------------------------- controls
class Segmented(QWidget):
    """A segmented control: one of a few options, as a pill with a sliding highlight."""

    changed = Signal(int)

    def __init__(self, options, parent=None):
        super().__init__(parent)
        self.options = options
        self.index = 0
        self._x = 0.0
        self._anim = QVariantAnimation(self, duration=160)
        self._anim.valueChanged.connect(lambda v: (setattr(self, "_x", v), self.update()))
        self.setFont(T.font(T.LABEL, QFont.DemiBold))
        self.setCursor(Qt.PointingHandCursor)
        self.setFixedHeight(32)
        w = max(self.fontMetrics().horizontalAdvance(o) for o in options) + 32
        self._seg = w
        self.setFixedWidth(w * len(options) + 6)

    def set_index(self, i, animate=True):
        if i == self.index:
            return
        self.index = i
        if animate:
            self._anim.stop()
            self._anim.setStartValue(self._x)
            self._anim.setEndValue(float(i))
            self._anim.start()
        else:
            self._x = float(i)
            self.update()

    def mousePressEvent(self, e):
        i = int((e.position().x() - 3) // self._seg)
        if 0 <= i < len(self.options) and i != self.index:
            self.set_index(i)
            self.changed.emit(i)

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        r = QRectF(self.rect()).adjusted(0.5, 0.5, -0.5, -0.5)
        p.setPen(QPen(_c(T.STROKE), 1))
        p.setBrush(_c(T.BG))
        p.drawRoundedRect(r, 8, 8)
        hl = QRectF(3 + self._x * self._seg, 3, self._seg, self.height() - 6)
        p.setPen(Qt.NoPen)
        p.setBrush(_c(T.HOVER))
        p.drawRoundedRect(hl, 6, 6)
        for i, o in enumerate(self.options):
            p.setPen(_c(T.INK if i == self.index else T.INK_2))
            p.drawText(QRectF(3 + i * self._seg, 0, self._seg, self.height()), Qt.AlignCenter, o)


class Toggle(QWidget):
    toggled = Signal(bool)

    def __init__(self, checked=False, parent=None):
        super().__init__(parent)
        self._on = checked
        self._pos = 1.0 if checked else 0.0
        self.setFixedSize(40, 22)
        self.setCursor(Qt.PointingHandCursor)
        self._anim = QVariantAnimation(self, duration=150)
        self._anim.valueChanged.connect(lambda v: (setattr(self, "_pos", v), self.update()))

    def isChecked(self):
        return self._on

    def setChecked(self, on, animate=False):
        if on == self._on:
            return
        self._on = on
        if animate:
            self._anim.stop()
            self._anim.setStartValue(self._pos)
            self._anim.setEndValue(1.0 if on else 0.0)
            self._anim.start()
        else:
            self._pos = 1.0 if on else 0.0
            self.update()

    def mouseReleaseEvent(self, e):
        if e.button() == Qt.LeftButton and self.isEnabled():
            self.setChecked(not self._on, animate=True)
            self.toggled.emit(self._on)

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        r = QRectF(self.rect()).adjusted(1, 1, -1, -1)
        off, on = _c(T.PRESSED), _c(T.ACCENT)
        t = self._pos
        mix = QColor.fromRgbF(off.redF() + (on.redF() - off.redF()) * t, off.greenF() + (on.greenF() - off.greenF()) * t,
                              off.blueF() + (on.blueF() - off.blueF()) * t)
        if not self.isEnabled():
            mix.setAlphaF(0.4)
        p.setPen(Qt.NoPen)
        p.setBrush(mix)
        p.drawRoundedRect(r, r.height() / 2, r.height() / 2)
        d = r.height() - 6
        p.setBrush(_c("#FFFFFF"))
        p.drawEllipse(QRectF(r.left() + 3 + (r.width() - d - 6) * t, r.top() + 3, d, d))


class SliderField(QWidget):
    """Label, slider and an editable value with its unit (e.g. 1.20 mm)."""

    valueChanged = Signal(float)

    def __init__(self, label_text, lo, hi, step, unit="mm", decimals=2, parent=None):
        super().__init__(parent)
        self.lo, self.step = lo, step
        v = QVBoxLayout(self)
        v.setContentsMargins(0, 0, 0, 0)
        v.setSpacing(6)
        head = QHBoxLayout()
        head.addWidget(text(label_text, "Secondary"))
        head.addStretch(1)
        self.box = QDoubleSpinBox()
        self.box.setRange(lo, hi)
        self.box.setSingleStep(step)
        self.box.setDecimals(decimals)
        self.box.setAlignment(Qt.AlignRight)
        self.box.setFixedWidth(64)
        self.box.setFont(T.font(T.LABEL, QFont.DemiBold, tabular=True))
        head.addWidget(self.box)
        if unit:
            u = text(unit, "Unit")
            u.setFixedWidth(22)
            head.addWidget(u)
        v.addLayout(head)
        self.slider = QSlider(Qt.Horizontal)
        self.slider.setRange(0, round((hi - lo) / step))
        v.addWidget(self.slider)
        self.slider.valueChanged.connect(self._from_slider)
        self.box.valueChanged.connect(self._from_box)

    def value(self):
        return round(self.box.value(), 4)

    def setValue(self, val):
        for w in (self.slider, self.box):
            w.blockSignals(True)
        self.box.setValue(val)
        self.slider.setValue(round((val - self.lo) / self.step))
        for w in (self.slider, self.box):
            w.blockSignals(False)

    def _from_slider(self, i):
        val = round(self.lo + i * self.step, 4)
        self.box.blockSignals(True)
        self.box.setValue(val)
        self.box.blockSignals(False)
        self.valueChanged.emit(val)

    def _from_box(self, val):
        self.slider.blockSignals(True)
        self.slider.setValue(round((val - self.lo) / self.step))
        self.slider.blockSignals(False)
        self.valueChanged.emit(round(val, 4))


# ------------------------------------------------------------------------------------------- navigation
class RailButton(QToolButton):
    def __init__(self, label_text, icon_name, parent=None):
        super().__init__(parent)
        self.label_text = label_text
        self.icon_name = icon_name
        self.setCheckable(True)
        self.setCursor(Qt.PointingHandCursor)
        self.setFixedSize(60, 58)
        self.setToolTip(label_text)
        self._hover = False

    def enterEvent(self, e):
        self._hover = True
        self.update()

    def leaveEvent(self, e):
        self._hover = False
        self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        on = self.isChecked()
        pill = QRectF(12, 6, 36, 28)
        if on or self._hover:
            p.setPen(Qt.NoPen)
            p.setBrush(_c(T.ACCENT_WASH.replace("0.14", "0.18")) if on else _c(T.RAISED))
            if on:
                p.setBrush(_c(T.ACCENT, 0.16))
            p.drawRoundedRect(pill, 9, 9)
        ink = T.ACCENT_INK if on else (T.INK if self._hover else T.INK_2)
        pm = icons.pixmap(self.icon_name, ink, 18)
        p.drawPixmap(QPointF(pill.center().x() - 9, pill.center().y() - 9), pm)
        p.setPen(_c(T.INK if on else T.INK_3))
        p.setFont(T.font(7.5, QFont.DemiBold if on else QFont.Normal))
        p.drawText(QRectF(0, 37, self.width(), 16), Qt.AlignCenter, self.label_text)


class IconButton(QToolButton):
    """A square icon-only button with a tooltip."""

    def __init__(self, icon_name, tip, size=34, parent=None):
        super().__init__(parent)
        self.icon_name = icon_name
        self.setToolTip(tip)
        self.setCursor(Qt.PointingHandCursor)
        self.setFixedSize(size, size)
        self._hover = False
        self.accent = False

    def enterEvent(self, e):
        self._hover = True
        self.update()

    def leaveEvent(self, e):
        self._hover = False
        self.update()

    def set_icon(self, name, accent=False):
        self.icon_name, self.accent = name, accent
        self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        r = QRectF(self.rect()).adjusted(0.5, 0.5, -0.5, -0.5)
        if self.accent:
            p.setPen(Qt.NoPen)
            p.setBrush(_c(T.ACCENT_HOVER if self._hover else T.ACCENT))
        else:
            p.setPen(QPen(_c(T.STROKE), 1))
            p.setBrush(_c(T.HOVER if self._hover else T.RAISED))
        p.drawRoundedRect(r, 8, 8)
        pm = icons.pixmap(self.icon_name, "#FFFFFF" if self.accent else T.INK, 16)
        p.drawPixmap(QPointF(r.center().x() - 8, r.center().y() - 8), pm)


class StatusDot(QWidget):
    """Dot + text, e.g. 'AK680 V2 · 990/s'."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self._text, self._color = "", T.INK_3
        self.setFont(T.font(T.LABEL))
        self.setFixedHeight(24)

    def set(self, message, color):
        if (message, color) != (self._text, self._color):
            self._text, self._color = message, color
            self.setToolTip(message)
            self.updateGeometry()
            self.update()

    def sizeHint(self):
        return QSize(self.fontMetrics().horizontalAdvance(self._text) + 22, 24)

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        cy = self.height() / 2
        g = QRadialGradient(QPointF(6, cy), 7)
        g.setColorAt(0, _c(self._color, 0.4))
        g.setColorAt(1, _c(self._color, 0))
        p.setPen(Qt.NoPen)
        p.setBrush(g)
        p.drawEllipse(QPointF(6, cy), 7, 7)
        p.setBrush(_c(self._color))
        p.drawEllipse(QPointF(6, cy), 3.5, 3.5)
        p.setPen(_c(T.INK_2))
        p.drawText(QRectF(18, 0, self.width() - 18, self.height()), Qt.AlignVCenter | Qt.AlignLeft,
                   self.fontMetrics().elidedText(self._text, Qt.ElideRight, int(self.width() - 20)))


# ------------------------------------------------------------------------------------------- profiles
class ProfileCard(QWidget):
    clicked = Signal(str)
    menu = Signal(str, QPointF)

    def __init__(self, name, active, parent=None):
        super().__init__(parent)
        self.name, self.active = name, active
        self.setFixedSize(212, 58)
        self.setCursor(Qt.PointingHandCursor)
        self._hover = False
        self.setToolTip(name)

    def enterEvent(self, e):
        self._hover = True
        self.update()

    def leaveEvent(self, e):
        self._hover = False
        self.update()

    def mouseReleaseEvent(self, e):
        if e.position().x() > self.width() - 34:
            self.menu.emit(self.name, e.globalPosition())
        elif e.button() == Qt.LeftButton:
            self.clicked.emit(self.name)

    def contextMenuEvent(self, e):
        self.menu.emit(self.name, QPointF(e.globalPos()))

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        r = QRectF(self.rect()).adjusted(0.5, 0.5, -0.5, -0.5)
        p.setPen(QPen(_c(T.ACCENT if self.active else T.STROKE), 1.2 if self.active else 1))
        p.setBrush(_c(T.HOVER if (self._hover and not self.active) else (T.RAISED if self.active else T.SURFACE)))
        p.drawRoundedRect(r, 11, 11)
        # avatar: initial on a soft tile
        av = QRectF(12, 13, 32, 32)
        p.setPen(Qt.NoPen)
        p.setBrush(_c(T.ACCENT, 0.2) if self.active else _c(T.PRESSED))
        p.drawRoundedRect(av, 9, 9)
        p.setPen(_c(T.ACCENT_INK if self.active else T.INK_2))
        p.setFont(T.font(11, QFont.Bold))
        p.drawText(av, Qt.AlignCenter, (self.name[:1] or "?").upper())
        p.setPen(_c(T.INK))
        p.setFont(T.font(T.BODY, QFont.DemiBold))
        fm = p.fontMetrics()
        p.drawText(QRectF(54, 10, self.width() - 90, 20), Qt.AlignLeft | Qt.AlignVCenter,
                   fm.elidedText(self.name, Qt.ElideRight, self.width() - 92))
        p.setFont(T.font(T.CAPTION))
        p.setPen(_c(T.ACCENT_INK if self.active else T.INK_3))
        p.drawText(QRectF(54, 29, 120, 18), Qt.AlignLeft | Qt.AlignVCenter, "Active" if self.active else "Profile")
        p.drawPixmap(QPointF(self.width() - 28, self.height() / 2 - 8), icons.pixmap("ellipsis-vertical", T.INK_3, 16))


class AddCard(QWidget):
    clicked = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setFixedSize(58, 58)
        self.setCursor(Qt.PointingHandCursor)
        self.setToolTip("New profile")
        self._hover = False

    def enterEvent(self, e):
        self._hover = True
        self.update()

    def leaveEvent(self, e):
        self._hover = False
        self.update()

    def mouseReleaseEvent(self, e):
        if e.button() == Qt.LeftButton:
            self.clicked.emit()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        r = QRectF(self.rect()).adjusted(0.5, 0.5, -0.5, -0.5)
        p.setPen(QPen(_c(T.STROKE), 1, Qt.DashLine))
        p.setBrush(_c(T.SURFACE if self._hover else T.BG))
        p.drawRoundedRect(r, 11, 11)
        p.drawPixmap(QPointF(r.center().x() - 9, r.center().y() - 9), icons.pixmap("plus", T.INK_2, 18))


# ------------------------------------------------------------------------------------------- keyboard
class KeyboardView(QWidget):
    """The AK680 V2 to scale. Keys fill from the bottom with their depth: accent when measured by
    the keyboard, amber when held from the key press. Mapped keys carry their input's glyph.

    When editable it is also a drop target: drop an input glyph on a key to map it, or drag a
    mapped key's glyph to another key to move the mapping."""

    keyClicked = Signal(int)
    keyHovered = Signal(object)                  # position or None
    dropped = Signal(int, str, object)           # key position, target, source position (move) or None

    MAX_WIDTH = 940  # the hero stays a comfortable size on wide windows instead of growing without limit

    def __init__(self, keymap, selectable=False, parent=None):
        super().__init__(parent)
        self.keymap = keymap
        self.selectable = selectable
        self.depths, self.shown, self.bindings = {}, {}, {}
        self.selected = self.hover = None
        self.highlight = set()                   # targets whose keys glow (chip hover)
        self.drop_pos, self.drop_target = None, None
        self.pops = {}                           # pos -> animation progress 0..1
        self.rects = {}
        self._press = None
        self.setMouseTracking(True)
        self.setAcceptDrops(selectable)
        self.setMinimumSize(620, 200)
        self.setMaximumWidth(self.MAX_WIDTH)
        self.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)

    def hasHeightForWidth(self):
        return True

    def heightForWidth(self, w):
        return int(min(w, self.MAX_WIDTH) * 5 / WIDTH_UNITS)

    def sizeHint(self):
        return QSize(self.MAX_WIDTH, self.heightForWidth(self.MAX_WIDTH))

    # --- state ------------------------------------------------------------------------------
    def set_state(self, depths):
        self.depths = depths
        changed = False
        for pos in set(self.shown) | set(depths):
            target = depths.get(pos, (0.0, None))[0]
            cur = self.shown.get(pos, 0.0)
            new = target if abs(target - cur) < 0.01 else cur + (target - cur) * 0.6
            if new != cur:
                self.shown[pos] = new
                changed = True
        if changed:
            self.update()

    def set_bindings(self, bindings):
        if bindings != self.bindings:
            self.bindings = dict(bindings)
            self.update()

    def set_selected(self, pos):
        self.selected = pos
        self.update()

    def set_highlight(self, targets):
        targets = set(t for t in (targets or ()) if t)
        if targets != self.highlight:
            self.highlight = targets
            self.update()

    def pop(self, pos):
        """A short eased 'pop' ring on a key that was just mapped."""
        anim = QVariantAnimation(self, duration=420, startValue=0.0, endValue=1.0)
        anim.valueChanged.connect(lambda v, pos=pos: (self.pops.__setitem__(pos, v), self.update()))
        anim.finished.connect(lambda pos=pos: (self.pops.pop(pos, None), self.update()))
        anim.start()

    # --- geometry and painting ----------------------------------------------------------------
    def _layout(self):
        unit = min(self.width() / WIDTH_UNITS, self.height() / 5)
        x0 = (self.width() - unit * WIDTH_UNITS) / 2
        y0 = (self.height() - unit * 5) / 2
        gap = max(3.0, unit * 0.075)
        self.rects = {}
        for r, row in enumerate(ROWS):
            x = x0
            for pos, _, w in row:
                self.rects[pos] = QRectF(x + gap / 2, y0 + r * unit + gap / 2, w * unit - gap, unit - gap)
                x += w * unit
        return unit

    def paintEvent(self, _):
        from . import glyphs
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        unit = self._layout()
        legend_font = T.font(max(8.0, unit / 5.6), QFont.Medium)
        small_legend = T.font(max(7.0, unit / 7.0), QFont.Medium)
        radius = unit * 0.12
        dragging = self.drop_target is not None
        for row in ROWS:
            for pos, legend, w in row:
                rect = self.rects[pos]
                depth = self.shown.get(pos, 0.0)
                source = self.depths.get(pos, (0, None))[1]
                bound = self.bindings.get(pos)
                previewing = dragging and pos == self.drop_pos
                if previewing:
                    bound = self.drop_target
                selected = pos == self.selected
                hovered = (self.hover == pos and self.selectable) or previewing
                mappable = bool(self.keymap.get(pos, {}).get("hid"))
                path = QPainterPath()
                path.addRoundedRect(rect, radius, radius)
                p.fillPath(path, _c(T.HOVER if (hovered or selected) else T.KEY))
                if dragging and not mappable:
                    p.fillPath(path, _c("#000000", 0.35))
                if depth > 0.001:
                    fill = T.ACCENT if source == "stream" else T.HELD
                    h = rect.height() * min(1.0, depth)
                    fr = QRectF(rect.left(), rect.bottom() - h, rect.width(), h)
                    g = QLinearGradient(fr.topLeft(), fr.bottomLeft())
                    g.setColorAt(0, _c(fill, 0.95))
                    g.setColorAt(1, _c(fill, 0.6))
                    p.save()
                    p.setClipPath(path)
                    p.fillRect(fr, g)
                    p.restore()
                p.save()
                p.setClipPath(path)
                p.fillRect(QRectF(rect.left(), rect.bottom() - unit * 0.06, rect.width(), unit * 0.06),
                           _c("#000000", 0.25))
                p.restore()
                if pos in self.pops:
                    t = self.pops[pos]
                    grow = unit * 0.18 * t
                    p.setPen(QPen(_c(T.ACCENT, 0.7 * (1 - t)), 2))
                    p.setBrush(Qt.NoBrush)
                    p.drawRoundedRect(rect.adjusted(-grow, -grow, grow, grow), radius + grow, radius + grow)
                ring = None
                if previewing:
                    ring = (T.ACCENT, 2.2)
                elif selected:
                    ring = (T.ACCENT, 2.0)
                elif bound in self.highlight:
                    ring = (T.ACCENT_INK, 1.4)
                if ring:
                    p.setPen(QPen(_c(ring[0]), ring[1]))
                    p.setBrush(Qt.NoBrush)
                    p.drawRoundedRect(rect.adjusted(1, 1, -1, -1), radius, radius)
                pressed_hard = depth > 0.45
                legend_rect = QRectF(rect)
                if bound:
                    gs = min(unit * 0.4, rect.height() * 0.42)
                    legend_rect = QRectF(rect.left(), rect.top(), rect.width(), rect.height() - gs * 0.9)
                    grect = QRectF(rect.center().x() - gs / 2, rect.bottom() - gs - unit * 0.1, gs, gs)
                    p.setOpacity(0.6 if previewing else 1.0)
                    glyphs.draw(p, grect, bound, active=pressed_hard)
                    p.setOpacity(1.0)
                p.setPen(_c("#FFFFFF" if pressed_hard else (T.INK if (bound or selected) else T.INK_2)))
                p.setFont(legend_font if len(legend) <= 2 else small_legend)
                p.drawText(legend_rect, Qt.AlignCenter, legend)

    # --- interaction ----------------------------------------------------------------------------
    def _pos_at(self, point):
        for pos, r in self.rects.items():
            if r.contains(point):
                return pos
        return None

    def mousePressEvent(self, e):
        if self.selectable and e.button() == Qt.LeftButton:
            self._press = (e.position(), self._pos_at(e.position()))

    def mouseMoveEvent(self, e):
        from PySide6.QtWidgets import QApplication
        if self._press and e.buttons() & Qt.LeftButton:
            start, pos = self._press
            if (pos is not None and pos in self.bindings
                    and (e.position() - start).manhattanLength() > QApplication.startDragDistance()):
                self._press = None
                start_drag(self, self.bindings[pos], from_pos=pos)
                return
        pos = self._pos_at(e.position())
        if pos != self.hover:
            self.hover = pos
            self.keyHovered.emit(pos)
            self.setCursor(Qt.PointingHandCursor if (pos is not None and self.selectable) else Qt.ArrowCursor)
            if self.selectable:
                self.update()

    def mouseReleaseEvent(self, e):
        if self._press and e.button() == Qt.LeftButton:
            _, pos = self._press
            self._press = None
            if pos is not None and pos == self._pos_at(e.position()):
                self.keyClicked.emit(pos)

    def leaveEvent(self, _):
        self.hover = None
        self.keyHovered.emit(None)
        self.update()

    @staticmethod
    def _payload(e):
        if not e.mimeData().hasFormat(MIME):
            return None, None
        raw = bytes(e.mimeData().data(MIME)).decode()
        target, _, src = raw.partition("|")
        return target, (int(src) if src else None)

    def dragEnterEvent(self, e):
        target, _ = self._payload(e)
        if target:
            self.drop_target = target
            e.acceptProposedAction()

    def dragMoveEvent(self, e):
        target, _ = self._payload(e)
        pos = self._pos_at(e.position())
        ok = bool(target) and pos is not None and bool(self.keymap.get(pos, {}).get("hid"))
        if (pos if ok else None) != self.drop_pos:
            self.drop_pos = pos if ok else None
            self.update()
        if ok:
            e.acceptProposedAction()
        else:
            e.ignore()

    def dragLeaveEvent(self, _):
        self.drop_pos = self.drop_target = None
        self.update()

    def dropEvent(self, e):
        target, src = self._payload(e)
        pos = self.drop_pos
        self.drop_pos = self.drop_target = None
        self.update()
        if target and pos is not None:
            e.acceptProposedAction()
            self.dropped.emit(pos, target, src)
            self.pop(pos)


# ------------------------------------------------------------------------------------------- controller
BITS = {"A": 0x1000, "B": 0x2000, "X": 0x4000, "Y": 0x8000, "LB": 0x0100, "RB": 0x0200, "VIEW": 0x0020,
        "MENU": 0x0010, "LS": 0x0040, "RS": 0x0080, "UP": 0x0001, "DOWN": 0x0002, "LEFT": 0x0004,
        "RIGHT": 0x0008, "GUIDE": 0x0400}


class ControllerView(QWidget):
    """A controller with every input in its real place, live."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.state = (0, 0.0, 0.0, (0.0, 0.0, 0.0, 0.0))
        self.setMinimumSize(420, 250)
        self.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)

    def set_state(self, out):
        b, lt, rt, ax = out
        new = (b, round(lt, 3), round(rt, 3), tuple(round(a, 3) for a in ax))
        if new != self.state:
            self.state = new
            self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        b, lt, rt, (lx, ly, rx, ry) = self.state
        W, H = self.width(), self.height()
        s = min(W / 1.6, H / 0.98)
        cx, top = W / 2, (H - s * 0.95) / 2
        on = lambda n: bool(b & BITS[n])
        for side, val, bump in ((-1, lt, "LB"), (1, rt, "RB")):
            x = cx + side * s * 0.5
            self._trigger(p, QRectF(x - s * 0.19, top, s * 0.38, s * 0.07), val, "LT" if side < 0 else "RT")
            self._pill(p, QRectF(x - s * 0.17, top + s * 0.11, s * 0.34, s * 0.055), on(bump), bump)
        bt = top + s * 0.21
        body = QPainterPath()
        body.addRoundedRect(QRectF(cx - s * 0.7, bt, s * 1.4, s * 0.44), s * 0.2, s * 0.2)
        for side in (-1, 1):
            grip = QPainterPath()
            grip.addEllipse(QPointF(cx + side * s * 0.53, bt + s * 0.45), s * 0.21, s * 0.27)
            body = body.united(grip)
        bridge = QPainterPath()
        bridge.addRoundedRect(QRectF(cx - s * 0.4, bt + s * 0.2, s * 0.8, s * 0.33), s * 0.12, s * 0.12)
        body = body.united(bridge)
        g = QLinearGradient(QPointF(cx, bt), QPointF(cx, bt + s * 0.74))
        g.setColorAt(0, _c("#242424"))
        g.setColorAt(1, _c("#1B1B1B"))
        p.setPen(QPen(_c(T.STROKE), 1))
        p.setBrush(g)
        p.drawPath(body)
        self._stick(p, QPointF(cx - s * 0.46, bt + s * 0.2), s * 0.115, lx, ly, on("LS"))
        self._stick(p, QPointF(cx + s * 0.21, bt + s * 0.42), s * 0.115, rx, ry, on("RS"))
        dc = QPointF(cx - s * 0.21, bt + s * 0.42)
        k = s * 0.048
        p.setPen(Qt.NoPen)
        p.setBrush(_c("#151515"))
        p.drawEllipse(dc, k * 2.0, k * 2.0)
        for name, dx, dy in (("UP", 0, -1), ("DOWN", 0, 1), ("LEFT", -1, 0), ("RIGHT", 1, 0)):
            p.setBrush(_c(T.ACCENT) if on(name) else _c(T.PRESSED))
            p.drawRoundedRect(QRectF(dc.x() + dx * k - k / 2, dc.y() + dy * k - k / 2, k, k), 2, 2)
        p.setBrush(_c(T.PRESSED))
        p.drawRect(QRectF(dc.x() - k / 2, dc.y() - k / 2, k, k))
        fc = QPointF(cx + s * 0.46, bt + s * 0.2)
        d = s * 0.085
        for name, dx, dy in (("Y", 0, -1), ("A", 0, 1), ("X", -1, 0), ("B", 1, 0)):
            c = QPointF(fc.x() + dx * d, fc.y() + dy * d)
            col = T.FACE[name]
            pressed = on(name)
            if pressed:
                glow = QRadialGradient(c, s * 0.08)
                glow.setColorAt(0, _c(col, 0.4))
                glow.setColorAt(1, _c(col, 0))
                p.setPen(Qt.NoPen)
                p.setBrush(glow)
                p.drawEllipse(c, s * 0.08, s * 0.08)
            p.setPen(QPen(_c(col, 0.9 if pressed else 0.45), 1.2))
            p.setBrush(_c(col) if pressed else _c("#151515"))
            p.drawEllipse(c, s * 0.04, s * 0.04)
            p.setPen(_c("#101010" if pressed else col))
            p.setFont(T.font(max(7.0, s * 0.03), QFont.Bold))
            p.drawText(QRectF(c.x() - 10, c.y() - 10, 20, 20), Qt.AlignCenter, name)
        for name, dx in (("VIEW", -0.1), ("MENU", 0.1)):
            r = QRectF(cx + dx * s - s * 0.025, bt + s * 0.2 - s * 0.016, s * 0.05, s * 0.032)
            p.setPen(Qt.NoPen)
            p.setBrush(_c(T.ACCENT) if on(name) else _c(T.PRESSED))
            p.drawRoundedRect(r, r.height() / 2, r.height() / 2)
        p.setBrush(_c(T.ACCENT) if on("GUIDE") else _c(T.PRESSED))
        p.drawEllipse(QPointF(cx, bt + s * 0.075), s * 0.034, s * 0.034)

    def _trigger(self, p, r, v, label):
        path = QPainterPath()
        path.addRoundedRect(r, r.height() / 2, r.height() / 2)
        p.fillPath(path, _c(T.RAISED))
        if v > 0.002:
            p.save()
            p.setClipPath(path)
            p.fillRect(QRectF(r.left(), r.top(), r.width() * v, r.height()), _c(T.ACCENT))
            p.restore()
        p.setPen(_c(T.INK if v > 0.45 else T.INK_2))
        p.setFont(T.font(8, QFont.Bold, tabular=True))
        p.drawText(r, Qt.AlignCenter, f"{label}  {int(round(v * 100))}%")

    def _pill(self, p, r, pressed, label):
        p.setPen(Qt.NoPen)
        p.setBrush(_c(T.ACCENT) if pressed else _c(T.RAISED))
        p.drawRoundedRect(r, r.height() / 2, r.height() / 2)
        p.setPen(_c("#FFFFFF" if pressed else T.INK_2))
        p.setFont(T.font(7.5, QFont.Bold))
        p.drawText(r, Qt.AlignCenter, label)

    def _stick(self, p, c, r, x, y, clicked):
        p.setPen(QPen(_c(T.STROKE), 1))
        p.setBrush(_c("#141414"))
        p.drawEllipse(c, r * 1.3, r * 1.3)
        active = abs(x) > 0.01 or abs(y) > 0.01
        dot = QPointF(c.x() + x * r * 0.65, c.y() - y * r * 0.65)
        if active:
            glow = QRadialGradient(dot, r * 1.1)
            glow.setColorAt(0, _c(T.ACCENT, 0.3))
            glow.setColorAt(1, _c(T.ACCENT, 0))
            p.setPen(Qt.NoPen)
            p.setBrush(glow)
            p.drawEllipse(dot, r * 1.1, r * 1.1)
        g = QRadialGradient(QPointF(dot.x() - r * 0.2, dot.y() - r * 0.25), r)
        g.setColorAt(0, _c("#8DB4FF" if active else "#3C3C3C"))
        g.setColorAt(1, _c(T.ACCENT if active else "#262626"))
        p.setPen(QPen(_c(T.ACCENT if clicked else "#000000", 0.9 if clicked else 0.3), 2 if clicked else 1))
        p.setBrush(g)
        p.drawEllipse(dot, r * 0.64, r * 0.64)


# ------------------------------------------------------------------------------------------- analysis
class AxisMeter(QWidget):
    """A labelled value with a meter: bipolar (-1..1, centre baseline) for stick axes, 0..1 for triggers."""

    def __init__(self, label_text, bipolar, parent=None):
        super().__init__(parent)
        self.label_text, self.bipolar = label_text, bipolar
        self.value = 0.0
        self.setFixedHeight(52)
        self.setMinimumWidth(120)

    def set_value(self, v):
        if abs(v - self.value) > 0.001:
            self.value = v
            self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        p.setPen(_c(T.INK_3))
        p.setFont(T.font(T.CAPTION))
        p.drawText(QRectF(0, 0, self.width(), 16), Qt.AlignLeft | Qt.AlignVCenter, self.label_text)
        p.setPen(_c(T.INK))
        p.setFont(T.font(13, QFont.DemiBold, tabular=True))
        txt = f"{self.value:+.2f}" if self.bipolar else f"{self.value * 100:.0f}%"
        p.drawText(QRectF(0, 0, self.width(), 30), Qt.AlignRight | Qt.AlignTop, txt)
        track = QRectF(0, self.height() - 10, self.width(), 6)
        p.setPen(Qt.NoPen)
        p.setBrush(_c(T.PRESSED))
        p.drawRoundedRect(track, 3, 3)
        p.setBrush(_c(T.ACCENT))
        if self.bipolar:
            mid = track.center().x()
            w = track.width() / 2 * max(-1.0, min(1.0, self.value))
            fr = QRectF(min(mid, mid + w), track.top(), abs(w), track.height())
            p.drawRoundedRect(fr, 3, 3)
            p.setBrush(_c(T.INK_3))
            p.drawRect(QRectF(mid - 0.5, track.top() - 2, 1, track.height() + 4))
        else:
            p.drawRoundedRect(QRectF(track.left(), track.top(), track.width() * max(0.0, min(1.0, self.value)),
                                     track.height()), 3, 3)


class LiveChart(QWidget):
    """Real-time line chart of up to two series over the last few seconds (one y-axis).

    Follows the dataviz method: 2 px lines, solid hairline grid, a legend for 2 series,
    end dots with a surface ring, neutral text, and a crosshair with values on hover."""

    def __init__(self, title, series, lo, hi, seconds=5.0, parent=None):
        super().__init__(parent)
        self.title, self.series = title, series          # [(name, colour)]
        self.lo, self.hi, self.seconds = lo, hi, seconds
        self.data = collections.deque()                   # (t, (v1, v2))
        self.hover_x = None
        self.setMouseTracking(True)
        self.setMinimumHeight(150)
        self.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)

    def push(self, t, values):
        self.data.append((t, values))
        while self.data and t - self.data[0][0] > self.seconds:
            self.data.popleft()
        self.update()

    def mouseMoveEvent(self, e):
        self.hover_x = e.position().x()
        self.update()

    def leaveEvent(self, _):
        self.hover_x = None
        self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        p.setPen(_c(T.INK))
        p.setFont(T.font(T.LABEL, QFont.DemiBold))
        p.drawText(QRectF(0, 0, self.width(), 18), Qt.AlignLeft | Qt.AlignVCenter, self.title)
        # legend (two series): swatch + neutral text
        p.setFont(T.font(T.CAPTION))
        x = self.width()
        for name, col in reversed(self.series):
            w = p.fontMetrics().horizontalAdvance(name)
            x -= w
            p.setPen(_c(T.INK_2))
            p.drawText(QRectF(x, 0, w, 18), Qt.AlignVCenter, name)
            x -= 16
            p.setPen(QPen(_c(col), 2, Qt.SolidLine, Qt.RoundCap))
            p.drawLine(QPointF(x, 9), QPointF(x + 10, 9))
            x -= 14
        area = QRectF(34, 28, self.width() - 40, self.height() - 34)
        to_y = lambda v: area.bottom() - (v - self.lo) / (self.hi - self.lo) * area.height()
        ticks = (self.lo, (self.lo + self.hi) / 2, self.hi)
        p.setFont(T.font(8, tabular=True))
        for v in ticks:
            y = to_y(v)
            base = abs(v) < 1e-9 and self.lo < 0
            p.setPen(QPen(_c("#3A3A3A" if base else T.STROKE_SOFT), 1))
            p.drawLine(QPointF(area.left(), y), QPointF(area.right(), y))
            p.setPen(_c(T.INK_3))
            if self.lo < 0:
                lab = "0" if abs(v) < 1e-9 else f"{v:+.0f}"
            else:
                lab = f"{v * 100:.0f}%"
            p.drawText(QRectF(0, y - 7, 28, 14), Qt.AlignRight | Qt.AlignVCenter, lab)
        if len(self.data) < 2:
            return
        t_end = self.data[-1][0]
        to_x = lambda t: area.right() - (t_end - t) / self.seconds * area.width()
        for i, (name, col) in enumerate(self.series):
            path = QPainterPath()
            for j, (t, vals) in enumerate(self.data):
                pt = QPointF(to_x(t), to_y(vals[i]))
                if j == 0:
                    path.moveTo(pt)
                else:
                    path.lineTo(pt)
            p.setPen(QPen(_c(col), 2, Qt.SolidLine, Qt.RoundCap, Qt.RoundJoin))
            p.setBrush(Qt.NoBrush)
            p.drawPath(path)
            end = QPointF(to_x(t_end), to_y(self.data[-1][1][i]))
            p.setPen(QPen(_c(T.SURFACE), 2))
            p.setBrush(_c(col))
            p.drawEllipse(end, 4, 4)
        if self.hover_x is not None and area.left() <= self.hover_x <= area.right():
            t = t_end - (area.right() - self.hover_x) / area.width() * self.seconds
            sample = min(self.data, key=lambda d: abs(d[0] - t))
            hx = to_x(sample[0])
            p.setPen(QPen(_c(T.INK_3), 1))
            p.drawLine(QPointF(hx, area.top()), QPointF(hx, area.bottom()))
            lines = [f"{t_end - sample[0]:.2f} s ago"] + [f"{n}  {sample[1][i]:+.2f}" if self.lo < 0 else
                                                          f"{n}  {sample[1][i] * 100:.0f}%"
                                                          for i, (n, _) in enumerate(self.series)]
            p.setFont(T.font(T.CAPTION, tabular=True))
            fm = p.fontMetrics()
            w = max(fm.horizontalAdvance(s) for s in lines) + 30
            h = len(lines) * 17 + 10
            bx = hx + 10 if hx + 10 + w < area.right() else hx - 10 - w
            box = QRectF(bx, area.top() + 4, w, h)
            p.setPen(QPen(_c(T.STROKE), 1))
            p.setBrush(_c(T.RAISED))
            p.drawRoundedRect(box, 8, 8)
            for k, s in enumerate(lines):
                y = box.top() + 5 + k * 17
                if k:
                    p.setPen(QPen(_c(self.series[k - 1][1]), 2, Qt.SolidLine, Qt.RoundCap))
                    p.drawLine(QPointF(box.left() + 10, y + 8.5), QPointF(box.left() + 18, y + 8.5))
                p.setPen(_c(T.INK if k else T.INK_3))
                p.drawText(QRectF(box.left() + 24, y, w - 28, 17), Qt.AlignVCenter, s)


# ------------------------------------------------------------------------------------------- mapping
class TargetPicker(QWidget):
    """Controller inputs as grouped chips; one (or none) is chosen."""

    chosen = Signal(object)
    hovered = Signal(object)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.group = QButtonGroup(self)
        self.buttons = {}
        self._hint = None
        grid = QGridLayout(self)
        grid.setContentsMargins(0, 0, 0, 0)
        grid.setHorizontalSpacing(6)
        grid.setVerticalSpacing(7)
        grid.setColumnMinimumWidth(0, 78)
        rows = TARGET_GROUPS + [("None", [None])]
        for r, (title, targets) in enumerate(rows):
            cap = text(title, "Tertiary")
            grid.addWidget(cap, r, 0, Qt.AlignVCenter)
            for i, t in enumerate(targets):
                b = DragChip(TARGET_SHORT.get(t, t) if t else "Not mapped", t, self.hovered.emit)
                b.setObjectName("Chip")
                b.setCheckable(True)
                b.setCursor(Qt.PointingHandCursor)
                b.clicked.connect(lambda _=None, t=t: self.chosen.emit(t))
                self.group.addButton(b)
                self.buttons[t] = b
                if t is None:
                    grid.addWidget(b, r, 1, 1, 2)
                else:
                    grid.addWidget(b, r, 1 + i)
        for c in range(1, 5):
            grid.setColumnStretch(c, 1)

    def set_value(self, target):
        b = self.buttons.get(target)
        if b:
            b.setChecked(True)

    def set_hint(self, target):
        """Outline the chip for `target` (the input of the key under the mouse)."""
        if target == self._hint:
            return
        for t in (self._hint, target):
            b = self.buttons.get(t)
            if b is not None and t is not None:
                b.setProperty("hint", t == target)
                b.style().unpolish(b)
                b.style().polish(b)
        self._hint = target

    def setEnabled(self, on):
        # Chips stay draggable onto any key; clicking only applies when a mappable key is selected.
        for b in self.buttons.values():
            b.setProperty("dim", not on)


class DragChip(QPushButton):
    """A chip that can also be dragged onto a key."""

    def __init__(self, label_text, target, on_hover=None, parent=None):
        super().__init__(label_text, parent)
        self.target = target
        self.on_hover = on_hover
        self._press = None
        if target:
            from . import glyphs
            from .layout import TARGETS_LONG
            from PySide6.QtGui import QIcon
            self.setText("")
            self.setIcon(QIcon(glyphs.pixmap(target, 24)))
            self.setIconSize(QSize(24, 24))
            self.setMinimumHeight(38)
            self.setToolTip(TARGETS_LONG.get(target, target) + "\nClick to map the selected key, or drag onto any key")

    def enterEvent(self, e):
        if self.on_hover:
            self.on_hover(self.target)
        super().enterEvent(e)

    def leaveEvent(self, e):
        if self.on_hover:
            self.on_hover(None)
        super().leaveEvent(e)

    def mousePressEvent(self, e):
        self._press = e.position()
        super().mousePressEvent(e)

    def mouseMoveEvent(self, e):
        from PySide6.QtWidgets import QApplication
        if (self.target and self._press is not None and e.buttons() & Qt.LeftButton
                and (e.position() - self._press).manhattanLength() > QApplication.startDragDistance()):
            self._press = None
            self.setDown(False)
            start_drag(self, self.target)
            return
        super().mouseMoveEvent(e)


class CurveView(QWidget):
    """Output vs key travel; dead zone shaded, press point marked, live key position as a dot."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.curve = self.fn = None
        self.live_mm = None
        self.press_mm = None
        self.setMinimumSize(260, 170)
        self.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)

    def set_curve(self, curve, fn, press=False):
        self.curve, self.fn = curve, fn
        self.press_mm = curve["press_mm"] if (press and curve) else None
        self.update()

    def set_live(self, mm):
        if mm != self.live_mm:
            self.live_mm = mm
            self.update()

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.Antialiasing)
        area = QRectF(34, 6, self.width() - 40, self.height() - 30)
        to_x = lambda mm: area.left() + area.width() * mm / 3.4
        to_y = lambda v: area.bottom() - area.height() * v
        p.setFont(T.font(8, tabular=True))
        for mm in (0, 1, 2, 3):
            x = to_x(mm)
            p.setPen(QPen(_c(T.STROKE_SOFT), 1))
            p.drawLine(QPointF(x, area.top()), QPointF(x, area.bottom()))
            p.setPen(_c(T.INK_3))
            p.drawText(QRectF(x - 22, area.bottom() + 6, 44, 14), Qt.AlignCenter, f"{mm} mm")
        for v in (0, 0.5, 1):
            y = to_y(v)
            p.setPen(QPen(_c(T.STROKE_SOFT), 1))
            p.drawLine(QPointF(area.left(), y), QPointF(area.right(), y))
            p.setPen(_c(T.INK_3))
            p.drawText(QRectF(0, y - 7, 28, 14), Qt.AlignRight | Qt.AlignVCenter, f"{int(v * 100)}%")
        if not self.curve:
            return
        dz = to_x(self.curve["deadzone_mm"])
        p.fillRect(QRectF(area.left(), area.top(), dz - area.left(), area.height()), _c("#FFFFFF", 0.03))
        line, fill = QPainterPath(), QPainterPath()
        fill.moveTo(area.left(), area.bottom())
        for i in range(201):
            mm = 3.4 * i / 200
            pt = QPointF(to_x(mm), to_y(self.fn(mm / 3.4)))
            if i == 0:
                line.moveTo(pt)
            else:
                line.lineTo(pt)
            fill.lineTo(pt)
        fill.lineTo(area.right(), area.bottom())
        fill.closeSubpath()
        p.fillPath(fill, _c(T.ACCENT, 0.10))
        p.setPen(QPen(_c(T.ACCENT), 2, Qt.SolidLine, Qt.RoundCap, Qt.RoundJoin))
        p.setBrush(Qt.NoBrush)
        p.drawPath(line)
        if self.press_mm is not None:
            x = to_x(self.press_mm)
            p.setPen(QPen(_c(T.HELD), 1.5))
            p.drawLine(QPointF(x, area.top()), QPointF(x, area.bottom()))
        if self.live_mm is not None and self.live_mm > 0.005:
            x = to_x(min(3.4, self.live_mm))
            y = to_y(self.fn(min(1.0, self.live_mm / 3.4)))
            p.setPen(QPen(_c(T.INK_3), 1))
            p.drawLine(QPointF(x, area.bottom()), QPointF(x, y))
            p.setPen(QPen(_c(T.SURFACE), 2))
            p.setBrush(_c("#FFFFFF"))
            p.drawEllipse(QPointF(x, y), 5, 5)


class Legend(QWidget):
    def __init__(self, items, parent=None):
        super().__init__(parent)
        h = QHBoxLayout(self)
        h.setContentsMargins(0, 0, 0, 0)
        h.setSpacing(16)
        for col, label_text in items:
            sw = QLabel()
            sw.setFixedSize(8, 8)
            sw.setStyleSheet(f"background: {col}; border-radius: 2px;")
            pair = QHBoxLayout()
            pair.setSpacing(6)
            pair.addWidget(sw)
            pair.addWidget(text(label_text, "Tertiary"))
            h.addLayout(pair)


# ------------------------------------------------------------------------------------------- art
def app_icon_pixmap(size):
    """Depthpad mark: a keycap whose lower part is 'pressed' (filled), on an accent tile."""
    pm = QPixmap(size, size)
    pm.fill(Qt.transparent)
    p = QPainter(pm)
    p.setRenderHint(QPainter.Antialiasing)
    s = float(size)
    tile = QRectF(s * 0.04, s * 0.04, s * 0.92, s * 0.92)
    g = QLinearGradient(tile.topLeft(), tile.bottomRight())
    g.setColorAt(0, _c("#6B9BFF"))
    g.setColorAt(1, _c("#3D5CE0"))
    p.setPen(Qt.NoPen)
    p.setBrush(g)
    p.drawRoundedRect(tile, s * 0.24, s * 0.24)
    cap = QRectF(s * 0.27, s * 0.25, s * 0.46, s * 0.5)
    path = QPainterPath()
    path.addRoundedRect(cap, s * 0.09, s * 0.09)
    p.setPen(QPen(_c("#FFFFFF"), max(1.0, s * 0.055)))
    p.setBrush(Qt.NoBrush)
    p.drawPath(path)
    p.save()
    p.setClipPath(path)
    p.fillRect(QRectF(cap.left(), cap.top() + cap.height() * 0.46, cap.width(), cap.height() * 0.54), _c("#FFFFFF"))
    p.restore()
    p.end()
    return pm


def chevron_png(path, color=T.INK_2, size=20):
    pm = QPixmap(size, size)
    pm.fill(Qt.transparent)
    p = QPainter(pm)
    p.setRenderHint(QPainter.Antialiasing)
    p.setPen(QPen(_c(color), size * 0.11, Qt.SolidLine, Qt.RoundCap, Qt.RoundJoin))
    p.drawPolyline([QPointF(size * 0.25, size * 0.4), QPointF(size * 0.5, size * 0.64), QPointF(size * 0.75, size * 0.4)])
    p.end()
    pm.save(path)
    return path
