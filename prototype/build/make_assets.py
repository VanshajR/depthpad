"""Generate build assets: the .ico and the Windows version resource."""
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, ROOT)
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtGui import QGuiApplication  # noqa: E402

from depthpad import APP_NAME, VERSION  # noqa: E402

OUT = os.path.join(ROOT, "build", "assets")


def make_icon():
    from PySide6.QtGui import QImage
    from depthpad.widgets import app_icon_pixmap
    import struct, io
    from PySide6.QtCore import QBuffer, QIODevice
    sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    images = []
    for s in sizes:
        buf = QBuffer()
        buf.open(QIODevice.WriteOnly)
        app_icon_pixmap(s).toImage().save(buf, "PNG")
        images.append((s, bytes(buf.data())))
    # ICO container with PNG-compressed entries (supported since Windows Vista).
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries, blobs = b"", b""
    for s, data in images:
        entries += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(data), offset + len(blobs))
        blobs += data
    path = os.path.join(OUT, "depthpad.ico")
    with open(path, "wb") as f:
        f.write(header + entries + blobs)
    app_icon_pixmap(256).save(os.path.join(OUT, "depthpad-256.png"))
    return path


def make_version_file():
    nums = [int(x) for x in VERSION.split(".")] + [0]
    t = tuple(nums[:4])
    text = f"""VSVersionInfo(
  ffi=FixedFileInfo(filevers={t}, prodvers={t}, mask=0x3f, flags=0x0, OS=0x40004, fileType=0x1, subtype=0x0, date=(0, 0)),
  kids=[
    StringFileInfo([StringTable('040904B0', [
      StringStruct('CompanyName', '{APP_NAME}'),
      StringStruct('FileDescription', '{APP_NAME} - analog gamepad for the AJAZZ AK680 V2 HE'),
      StringStruct('FileVersion', '{VERSION}'),
      StringStruct('InternalName', '{APP_NAME}'),
      StringStruct('LegalCopyright', 'AGPL-3.0'),
      StringStruct('OriginalFilename', '{APP_NAME}.exe'),
      StringStruct('ProductName', '{APP_NAME}'),
      StringStruct('ProductVersion', '{VERSION}')])]),
    VarFileInfo([VarStruct('Translation', [1033, 1200])])
  ]
)
"""
    path = os.path.join(OUT, "version_info.txt")
    with open(path, "w", encoding="utf8") as f:
        f.write(text)
    return path


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    app = QGuiApplication(sys.argv)
    print(make_icon())
    print(make_version_file())
