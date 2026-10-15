# Keyboard reports

A keyboard report is a file that describes your keyboard: its USB details and what it sends while you press keys. With it, someone can add support for the keyboard to Depthpad, even without owning one.

## Creating one

1. **Close the keyboard's own software** (its configurator app or web page), so it isn't changing the keyboard's mode.
2. Connect the keyboard **with the USB cable**.
3. Start the report tool, either:
   - in Depthpad: **Settings → Keyboard → Create report**, or
   - on its own: run `depthpad-probe.exe` from the [latest release](https://github.com/VanshajR/depthpad/releases). It needs nothing installed.
4. Pick your keyboard from the list.
5. Follow the four short steps. Each one tells you what to press and counts down while it records:
   1. don't touch the keyboard (4 s);
   2. press W slowly all the way down and back up, twice (10 s);
   3. hold W down, and press D part-way a few times (10 s);
   4. tap W, A, S, D, then two of them together (8 s).
6. The report is saved to your Desktop as `depthpad-report-VVVV-PPPP.json`.

Then [open a keyboard support request](https://github.com/VanshajR/depthpad/issues/new?template=keyboard-support.yml) and attach the file.

## What's in it, and what isn't

The report tool **only listens**. It sends nothing to the keyboard and changes no settings.

It contains:

- the keyboard's USB IDs, name and firmware revision, and your Windows version;
- each of its USB HID collections and their report descriptors (the keyboard's own description of what it sends);
- the reports the keyboard sent, and the keys you pressed, **only during the four steps**.

It doesn't contain anything you type outside the steps, or anything about other devices. It's plain JSON, so you can open it in Notepad and read it before sharing.

## If the report shows nothing

Many keyboards only send analog data after their own software switches it on (the AJAZZ AK680 does). The report then shows the keyboard's layout but no depth data, which is still useful, but the next step is to record that software talking to the keyboard:

1. Install [Wireshark](https://www.wireshark.org/) and, when asked, **USBPcap**.
2. Start Wireshark and capture on the USBPcap interface your keyboard is on.
3. Open the keyboard's software and go to the screen that shows key travel live (often called key test, calibration or actuation). Press a few keys slowly.
4. Stop the capture, save it as `.pcapng`, and attach it to the same issue.

Only do this with the keyboard's own software, and only press keys while it records: a USB capture records every key you press.
