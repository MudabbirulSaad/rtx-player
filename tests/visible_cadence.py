"""Count changes in an unobscured test video's desktop image using Windows Desktop Duplication."""
import argparse
import ctypes as c
from ctypes import wintypes as w
from fractions import Fraction
import json
from pathlib import Path
import re
import subprocess
import threading
import time


class Area:
    def __init__(self, pid):
        self.user=c.WinDLL("user32",use_last_error=True)
        u=self.user
        u.SetProcessDpiAwarenessContext(c.c_void_p(-4))
        # Output index zero is unambiguous on a single primary monitor.
        if u.GetSystemMetrics(80)!=1: raise RuntimeError("This test requires one active display")
        windows=[]
        callback=c.WINFUNCTYPE(w.BOOL,w.HWND,w.LPARAM)
        @callback
        def collect(hwnd,_):
            owner=w.DWORD()
            u.GetWindowThreadProcessId(hwnd,c.byref(owner))
            if owner.value==pid and u.IsWindowVisible(hwnd):
                title=c.create_unicode_buffer(160)
                u.GetWindowTextW(hwnd,title,len(title))
                if title.value=="RTX Player": windows.append(hwnd)
            return True
        u.EnumWindows(collect,0)
        if len(windows)!=1: raise RuntimeError("Expected one visible player window")
        u.FindWindowExW.argtypes=[w.HWND,w.HWND,w.LPCWSTR,w.LPCWSTR]
        u.FindWindowExW.restype=w.HWND
        self.hwnd=u.FindWindowExW(windows[0],None,"Static",None) or windows[0]
        self.pid=pid
        self.bounds=self.rect()
        left,top,right,bottom=self.bounds
        self.w=min(768,(right-left)//2)
        self.h=min(432,(bottom-top)//2)
        self.x=(left+right-self.w)//2
        self.y=(top+bottom-self.h)//2
        if self.w<64 or self.h<64: raise RuntimeError("Video window is too small")
        u.WindowFromPoint.argtypes=[w.POINT]
        u.WindowFromPoint.restype=w.HWND
        self.check()

    def rect(self):
        rect=w.RECT()
        if not self.user.GetWindowRect(self.hwnd,c.byref(rect)): raise c.WinError()
        return rect.left,rect.top,rect.right,rect.bottom

    def check(self):
        if self.rect()!=self.bounds: raise RuntimeError("Video surface moved during capture")
        for col in range(5):
            for row in range(3):
                point=w.POINT(self.x+col*(self.w-1)//4,self.y+row*(self.h-1)//2)
                owner=w.DWORD()
                self.user.GetWindowThreadProcessId(self.user.WindowFromPoint(point),c.byref(owner))
                if owner.value!=self.pid: raise RuntimeError("Video sample is obscured")


def measure(ffmpeg, pid, seconds):
    area=Area(pid)
    source=(f"ddagrab=output_idx=0:framerate=180:draw_mouse=0:dup_frames=1:"
            f"offset_x={area.x}:offset_y={area.y}:video_size={area.w}x{area.h}")
    args=[ffmpeg,"-nostdin","-hide_banner","-v","error","-f","lavfi","-i",source,
          "-t",str(seconds),"-vf","hwdownload,format=bgra,scale=96:64:flags=area,format=gray",
          "-fps_mode","passthrough","-f","framehash","-hash","sha256","pipe:1"]
    stop=threading.Event()
    issues=[]
    with subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                          creationflags=subprocess.CREATE_NO_WINDOW) as proc:
        def guard():
            try:
                while not stop.wait(0.1): area.check()
            except (RuntimeError,OSError) as exc:
                issues.append(str(exc))
                proc.kill()
        thread=threading.Thread(target=guard)
        thread.start()
        try:
            try: stream,stderr=proc.communicate(timeout=seconds+10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.communicate()
                raise RuntimeError("Desktop capture timed out")
        finally:
            stop.set()
            thread.join()
        if issues: raise RuntimeError(issues[0])
        if proc.returncode: raise RuntimeError(stderr.decode(errors="replace")[-1200:])
    previous=None
    changes=0
    pts=[]
    base=None
    for line in stream.decode("ascii").splitlines():
        match=re.match(r"#tb 0: (\d+/\d+)",line)
        if match: base=Fraction(match[1])
        if line.startswith("#") or not line.strip(): continue
        fields=[v.strip() for v in line.split(",")]
        pts.append(int(fields[2]))
        if previous is not None and fields[-1]!=previous: changes+=1
        previous=fields[-1]
    if not base or len(pts)<2: raise RuntimeError("No timed desktop samples")
    elapsed=float((pts[-1]-pts[0])*base)
    if elapsed<seconds-0.5: raise RuntimeError("Incomplete capture interval")
    return {"seconds":elapsed,"desktop_samples":len(pts),"changes":changes,
            "changed_images_per_second":changes/elapsed}


if __name__=="__main__":
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("ffmpeg",type=Path)
    p.add_argument("pid",type=int)
    p.add_argument("--seconds",type=int,choices=range(2,61),default=8)
    args=p.parse_args()
    print(json.dumps(measure(args.ffmpeg.resolve(strict=True),args.pid,args.seconds)))
