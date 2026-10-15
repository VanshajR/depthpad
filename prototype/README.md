# Python prototype

The first version of Depthpad, written in Python with Qt. The app in this repository replaced it; it's kept for reference and for its analysis tools (`tools/check_log.py`, `tools/replay_log.py`), which read Depthpad session recordings.

It needs Python 3.12 and `ViGEmClient.dll` (x64, from https://github.com/nefarius/ViGEmClient, MIT) in `ak680/vendor/`, which isn't in the repository.

```
python -m venv .venv
.venv\Scripts\pip install -r requirements.txt
.venv\Scripts\python run_depthpad.py
```
