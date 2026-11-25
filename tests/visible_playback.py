"""Verify a visible native player window using a generated testsrc2 clip on Windows."""
import argparse
import ctypes as ct
from ctypes import wintypes as wt
from pathlib import Path
import subprocess
import time

from window_process import close_player


def sample_window(pid):
    user = ct.WinDLL("user32", use_last_error=True)
    gdi = ct.WinDLL("gdi32", use_last_error=True)
    windows = []
    callback_type = ct.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)

    @callback_type
    def collect(hwnd, _):
        owner = wt.DWORD()
        user.GetWindowThreadProcessId(hwnd, ct.byref(owner))
        if owner.value == pid and user.IsWindowVisible(hwnd):
            windows.append(hwnd)
        return True

    user.EnumWindows(collect, 0)
    for hwnd in windows:
        title = ct.create_unicode_buffer(128)
        user.GetWindowTextW(hwnd, title, len(title))
        if title.value != "RTX Player":
            continue
        user.FindWindowExW.argtypes = [wt.HWND, wt.HWND, wt.LPCWSTR, wt.LPCWSTR]
        user.FindWindowExW.restype = wt.HWND
        # The desktop host puts WebView controls in a separate sibling window.
        hwnd = user.FindWindowExW(hwnd, None, "Static", None) or hwnd
        rect = wt.RECT()
        user.GetClientRect(hwnd, ct.byref(rect))
        if rect.right < 160 or rect.bottom < 90:
            continue
        origin = wt.POINT()
        user.ClientToScreen(hwnd, ct.byref(origin))
        user.ReleaseDC.argtypes = [wt.HWND, wt.HDC]
        user.GetDC.restype = wt.HDC
        dc = user.GetDC(None)
        pixels = []
        try:
            user.WindowFromPoint.argtypes = [wt.POINT]
            user.WindowFromPoint.restype = wt.HWND
            gdi.GetPixel.argtypes = [wt.HDC, ct.c_int, ct.c_int]
            for y in range(2, 16):
                for x in range(2, 30):
                    point = wt.POINT(origin.x + rect.right*x//32,
                                     origin.y + rect.bottom*y//18)
                    owner = wt.DWORD()
                    user.GetWindowThreadProcessId(user.WindowFromPoint(point), ct.byref(owner))
                    if owner.value != pid:
                        return []
                    pixel = gdi.GetPixel(dc, point.x, point.y)
                    if pixel == -1:
                        return []
                    pixels.append(tuple((pixel >> shift) & 255 for shift in (0, 8, 16)))
        finally:
            user.ReleaseDC(None, dc)
        return pixels
    return []


def verify_pid(pid, alive):
    ct.windll.user32.SetProcessDPIAware()
    previous = None
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        if not alive():
            raise RuntimeError("Player exited during visible-frame verification")
        pixels = sample_window(pid)
        if pixels and sum(max(p)-min(p) > 80 for p in pixels) > len(pixels)//3:
            if previous and sum(max(abs(a-b) for a, b in zip(p, q)) > 35
                                for p, q in zip(previous, pixels)) >= 4:
                print(f"Visible moving colors verified at {len(pixels)} sample points")
                return
            previous = pixels
        time.sleep(0.3)
    raise RuntimeError("Expected a visible, unobscured, moving color pattern")


def verify(exe, clip):
    with subprocess.Popen([exe, clip]) as player:
        try:
            verify_pid(player.pid, lambda: player.poll() is None)
        finally:
            close_player(player)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe", type=Path, nargs="?")
    parser.add_argument("clip", type=Path, nargs="?")
    parser.add_argument("--pid", type=int, help="inspect an already playing test window")
    args = parser.parse_args()
    if args.pid and not args.exe and not args.clip:
        verify_pid(args.pid, lambda: True)
    elif args.exe and args.clip and not args.pid:
        verify(args.exe.resolve(strict=True), args.clip.resolve(strict=True))
    else:
        parser.error("Provide a player and clip or a running player PID")
