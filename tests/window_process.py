"""Close a test-owned desktop window before releasing its child processes."""
import ctypes as ct
from ctypes import wintypes as wt
import subprocess


def close_player(player):
    if player.poll() is not None:
        return
    user = ct.windll.user32
    callback = ct.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
    user.PostMessageW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]

    @callback
    def close(hwnd, _):
        pid = wt.DWORD()
        user.GetWindowThreadProcessId(hwnd, ct.byref(pid))
        title = ct.create_unicode_buffer(256)
        user.GetWindowTextW(hwnd, title, len(title))
        if pid.value == player.pid and title.value == "RTX Player" and user.IsWindowVisible(hwnd):
            user.PostMessageW(hwnd, 0x0010, 0, 0)
            return False
        return True

    user.EnumWindows(close, 0)
    try:
        player.wait(timeout=5)
    except subprocess.TimeoutExpired:
        player.kill()
        player.wait()
        raise RuntimeError("Player did not close within five seconds")
