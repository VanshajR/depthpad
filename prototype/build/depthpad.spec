# PyInstaller spec: one-folder, windowed build of Depthpad.
# Build:  .venv\Scripts\python -m PyInstaller build\depthpad.spec --noconfirm --distpath dist --workpath build\work
import os

ROOT = os.path.abspath(os.path.join(SPECPATH, ".."))
ASSETS = os.path.join(ROOT, "build", "assets")

a = Analysis(
    [os.path.join(ROOT, "run_depthpad.py")],
    pathex=[ROOT],
    datas=[
        (os.path.join(ROOT, "ak680", "ak680v2_keymap.json"), "ak680"),
        (os.path.join(ROOT, "ak680", "vendor", "ViGEmClient.dll"), os.path.join("ak680", "vendor")),
        (os.path.join(ROOT, "ak680", "vendor", "ViGEmClient-LICENSE.txt"), os.path.join("ak680", "vendor")),
        (os.path.join(ROOT, "depthpad", "icons"), os.path.join("depthpad", "icons")),
        (os.path.join(ROOT, "LICENSE"), "."),
        (os.path.join(ROOT, "THIRD_PARTY_NOTICES.md"), "."),
    ],
    hiddenimports=[],
    excludes=["tkinter", "unittest", "pydoc", "PySide6.QtQml", "PySide6.QtQuick", "PySide6.QtWebEngineCore",
              "PySide6.QtWebEngineWidgets", "PySide6.QtMultimedia", "PySide6.Qt3DCore", "PySide6.QtPdf",
              "PySide6.QtCharts", "PySide6.QtDataVisualization", "PySide6.QtOpenGL", "PySide6.QtSql",
              "PySide6.QtTest", "PySide6.QtXml", "PySide6.QtBluetooth", "PySide6.QtPositioning",
              "PySide6.QtSerialPort", "PySide6.QtDesigner", "PySide6.QtHelp", "PySide6.QtUiTools"],
    noarchive=False,
)
# Qt pieces a Widgets app doesn't use (software OpenGL, QML/Quick, PDF, translations) and the plugins that
# would pull them back in.
DROP = ("opengl32sw.dll", "qt6quick", "qt6qml", "qt6pdf", "qt6opengl", "qt6virtualkeyboard", "translations",
        "qtvirtualkeyboard", "qpdf.dll", "qmltooling")
keep = lambda item: not any(d in item[0].replace("\\", "/").lower() for d in DROP)
a.binaries = [b for b in a.binaries if keep(b)]
a.datas = [d for d in a.datas if keep(d)]

pyz = PYZ(a.pure)
exe = EXE(
    pyz, a.scripts, [],
    exclude_binaries=True,
    name="Depthpad",
    icon=os.path.join(ASSETS, "depthpad.ico"),
    version=os.path.join(ASSETS, "version_info.txt"),
    console=False,
    upx=False,
)
coll = COLLECT(exe, a.binaries, a.datas, name="Depthpad", upx=False)
