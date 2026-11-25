//! Read HDR mode for the display containing a native window. Requires Windows 11 24H2 for Info2.
use std::io;
use std::mem::size_of;
use windows_sys::Win32::Devices::Display::*;
use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, LUID};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFOEXW, MonitorFromWindow,
};

#[repr(C)]
#[derive(Default)]
struct AdvancedColorInfo2 {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    flags: u32,
    encoding: u32,
    bits_per_channel: u32,
    active_mode: u32,
}

#[derive(Debug)]
pub struct DisplayState {
    pub hdr_supported: bool,
    pub hdr_enabled: bool,
    pub hdr_active: bool,
    pub bits_per_channel: u32,
    pub refresh_hz: Option<f64>,
}

fn header<T>(kind: i32, adapter: LUID, id: u32) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: kind,
        size: size_of::<T>() as u32,
        adapterId: adapter,
        id,
    }
}

fn active_paths() -> io::Result<Vec<DISPLAYCONFIG_PATH_INFO>> {
    // Monitor changes can invalidate the buffer sizes between the two calls.
    for _ in 0..3 {
        let (mut paths, mut modes) = (0, 0);
        let result =
            unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut paths, &mut modes) };
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
        let mut p = vec![DISPLAYCONFIG_PATH_INFO::default(); paths as usize];
        let mut m = vec![DISPLAYCONFIG_MODE_INFO::default(); modes as usize];
        let result = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut paths,
                p.as_mut_ptr(),
                &mut modes,
                m.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        };
        if result == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
        p.truncate(paths as usize);
        return Ok(p);
    }
    Err(io::Error::other(
        "Display topology changed during the query",
    ))
}

pub fn for_window(hwnd: usize) -> io::Result<DisplayState> {
    let monitor = unsafe { MonitorFromWindow(hwnd as _, MONITOR_DEFAULTTONEAREST) };
    let mut monitor_info = MONITORINFOEXW::default();
    monitor_info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    if unsafe { GetMonitorInfoW(monitor, &mut monitor_info.monitorInfo) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut target = None;
    for path in active_paths()? {
        let mut name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: header::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>(
                DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                path.sourceInfo.adapterId,
                path.sourceInfo.id,
            ),
            ..Default::default()
        };
        let result = unsafe { DisplayConfigGetDeviceInfo(&mut name.header) };
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result));
        }
        if name.viewGdiDeviceName == monitor_info.szDevice
            && target.replace(path.targetInfo).is_some()
        {
            return Err(io::Error::other(
                "Cloned outputs have separate HDR capabilities",
            ));
        }
    }
    let t = target.ok_or_else(|| io::Error::other("No active display target for window"))?;
    // DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO_2 is absent from older SDK bindings.
    let mut info = AdvancedColorInfo2 {
        header: header::<AdvancedColorInfo2>(15, t.adapterId, t.id),
        ..Default::default()
    };
    let result = unsafe { DisplayConfigGetDeviceInfo(&mut info.header) };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result));
    }
    Ok(DisplayState {
        hdr_supported: info.flags & (1 << 4) != 0,
        hdr_enabled: info.flags & (1 << 5) != 0,
        hdr_active: info.active_mode == 2,
        bits_per_channel: info.bits_per_channel,
        refresh_hz: (t.refreshRate.Denominator != 0)
            .then(|| f64::from(t.refreshRate.Numerator) / f64::from(t.refreshRate.Denominator)),
    })
}
