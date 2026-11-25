use nvml_wrapper::{Nvml, enum_wrappers::device::TemperatureSensor};
use serde::Serialize;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

#[derive(Clone, Default, Serialize)]
pub struct GpuStats {
    pub devices: Vec<DeviceStats>,
    pub driver: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct DeviceStats {
    pub index: u32,
    pub name: Option<String>,
    pub compute: Option<String>,
    pub total_memory: Option<u64>,
    pub used_memory: Option<u64>,
    pub gpu_percent: Option<u32>,
    pub decoder_percent: Option<u32>,
    pub temperature_c: Option<u32>,
    pub power_w: Option<f64>,
}

fn load() -> Result<Nvml, String> {
    let mut directory = [0u16; 260];
    let n = unsafe { GetSystemDirectoryW(directory.as_mut_ptr(), directory.len() as u32) } as usize;
    if n == 0 || n >= directory.len() {
        return Err("Windows system directory unavailable".into());
    }
    let path = PathBuf::from(std::ffi::OsString::from_wide(&directory[..n])).join("nvml.dll");
    Nvml::builder()
        .lib_path(path.as_os_str())
        .init()
        .map_err(|_| "NVIDIA management telemetry unavailable.".into())
}

fn sample(nv: &Nvml) -> Result<GpuStats, String> {
    let mut devices = Vec::new();
    for index in 0..nv.device_count().map_err(|e| e.to_string())? {
        let dev = nv.device_by_index(index).map_err(|e| e.to_string())?;
        let memory = dev.memory_info().ok();
        devices.push(DeviceStats {
            index,
            name: dev.name().ok(),
            compute: dev
                .cuda_compute_capability()
                .ok()
                .map(|c| format!("{}.{}", c.major, c.minor)),
            total_memory: memory.as_ref().map(|m| m.total),
            used_memory: memory.as_ref().map(|m| m.used),
            gpu_percent: dev.utilization_rates().ok().map(|v| v.gpu),
            decoder_percent: dev.decoder_utilization().ok().map(|v| v.utilization),
            temperature_c: dev.temperature(TemperatureSensor::Gpu).ok(),
            power_w: dev.power_usage().ok().map(|v| v as f64 / 1000.0),
        });
    }
    Ok(GpuStats {
        devices,
        driver: nv.sys_driver_version().ok(),
        error: None,
    })
}

pub struct Gpus {
    pub latest: Arc<Mutex<GpuStats>>,
    _lifetime: mpsc::Sender<()>,
}

impl Gpus {
    pub fn start() -> std::io::Result<Self> {
        let latest = Arc::new(Mutex::new(GpuStats::default()));
        let shared = latest.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("gpu-telemetry".into())
            .spawn(move || {
                let nv = load();
                loop {
                    let result = nv.as_ref().map_err(Clone::clone).and_then(sample);
                    if let Ok(mut slot) = shared.lock() {
                        *slot = result.unwrap_or_else(|error| GpuStats {
                            error: Some(error),
                            ..GpuStats::default()
                        });
                    }
                    if nv.is_err()
                        || !matches!(
                            rx.recv_timeout(Duration::from_secs(2)),
                            Err(mpsc::RecvTimeoutError::Timeout)
                        )
                    {
                        break;
                    }
                }
            })?;
        Ok(Self {
            latest,
            _lifetime: tx,
        })
    }
}

#[tauri::command]
pub fn gpu_stats(gpus: tauri::State<'_, Gpus>) -> Result<GpuStats, String> {
    gpus.latest
        .lock()
        .map(|g| g.clone())
        .map_err(|_| "GPU telemetry unavailable".into())
}
