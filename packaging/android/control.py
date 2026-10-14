#!/usr/bin/env python3
"""Enable the existing authenticated control protocol on a USB-attached debug APK.

Writes the token to a private local file and the app's private directory. Never
prints it. Set ANDROID_SERIAL when more than one adb device is attached.
"""
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys

PACKAGE = "ai.storyteller.photocraft"
PORT = 7878

def adb(*args, data=None):
    return subprocess.run(["adb", *args], input=data, check=True, capture_output=True).stdout

def main():
    if len(sys.argv) != 2 or sys.argv[1] not in ("enable", "disable"):
        raise SystemExit("Usage: packaging/android/control.py enable|disable")
    if sys.argv[1] == "disable":
        adb("shell", "run-as", PACKAGE, "rm", "-f", "files/control.json")
        subprocess.run(["adb", "forward", "--remove", f"tcp:{PORT}"], capture_output=True)
    else:
        token = secrets.token_hex(32)
        output = Path("target/android-apk/control-token")
        output.parent.mkdir(parents=True, exist_ok=True)
        fd = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w") as stream:
            stream.write(token + "\n")
        adb("shell", "run-as", PACKAGE, "mkdir", "-p", "files")
        adb("shell", "run-as", PACKAGE, "sh", "-c", "'cat > files/control.json'",
            data=json.dumps({"port": PORT, "token": token}).encode())
        adb("forward", f"tcp:{PORT}", f"tcp:{PORT}")
        print(f"Control token: {output}; forwarded localhost:{PORT}")
    print(f"Android control {sys.argv[1]}d. Save your work, then close and relaunch PhotoCraft to apply it.")

if __name__ == "__main__":
    main()
