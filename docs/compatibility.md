# Compatibility and limitations

The physical reference machine ran Windows 11 Home build 26300 with an RTX 4080 SUPER, NVIDIA driver 621.14, 16 GiB of device memory and a 3440 × 1440 display. Tests used local, generated media. Different clips, display modes and driver versions can give different results.

| Feature | Observed on the reference machine |
| --- | --- |
| Baseline video and audio | Moving H.264, HEVC, AV1 and VP9 sources. D3D11VA and WASAPI verified. Software decoding also produced moving video. |
| Anime4K | Measurable pixel change on a fixed enlarged frame. Disabling restored the original. |
| RIFE | Intermediate object positions, scene cuts and fractional durations verified for 24 → 48, 60 and 30 fps. Requires the separate NCNN/Vulkan runtime and SDR input. |
| NVIDIA VSR | Driver accepted the request and produced the chosen dimensions. Two fixed-frame comparisons found no measurable difference from conventional scaling on those inputs. |
| HDR playback | Native HDR and driver SDR conversion produced moving video. Renderer reported 10-bit BT.2020/PQ. The physical display link reported 8 bpc. |
| Enhanced export | HEVC PQ 10-bit encode and decode verified with software and NVENC. RIFE, Anime4K and inverse tone mapping produced all 960 expected frames from a 384-frame source. Interpolated export requires a zero-based, constant-rate video timeline. |
| Real-time target | Sustained 1080p SDR playback near 60 fps. Internal 4K144 reconstruction failed the real-time target. Native 4K source decoding succeeded. |
| Recovery | Missing or damaged RIFE model, cancelled setup and export, corrupt catalog, preference restart, safe-mode playback and repeated file switches exercised. |

Other RTX generations, AMD and Intel GPUs, Windows 10, multiple simultaneous displays and physical UHD/10-bit output await independent testing. There is no validated 4K144 HDR hardware configuration for this build. Direct TensorRT, Maxine and Real-ESRGAN providers are outside the available backends.

See the [benchmark conditions and timing distributions](performance.md).
