"""Lucide icons (ISC licence, https://lucide.dev), tinted and rendered crisp at any scale."""
import os
from functools import lru_cache

from PySide6.QtCore import QByteArray, QRectF, Qt
from PySide6.QtGui import QIcon, QPainter, QPixmap
from PySide6.QtSvg import QSvgRenderer

_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "icons")


@lru_cache(maxsize=None)
def _svg(name):
    with open(os.path.join(_DIR, name + ".svg"), encoding="utf8") as f:
        return f.read()


@lru_cache(maxsize=256)
def pixmap(name, color, size=18, stroke=1.75, dpr=2.0):
    data = _svg(name).replace("currentColor", color).replace('stroke-width="2"', f'stroke-width="{stroke}"')
    pm = QPixmap(int(size * dpr), int(size * dpr))
    pm.fill(Qt.transparent)
    p = QPainter(pm)
    p.setRenderHint(QPainter.Antialiasing)
    QSvgRenderer(QByteArray(data.encode())).render(p, QRectF(0, 0, size * dpr, size * dpr))
    p.end()
    pm.setDevicePixelRatio(dpr)
    return pm


def icon(name, color, size=18):
    return QIcon(pixmap(name, color, size))
