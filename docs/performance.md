# Windows playback measurements

The reference system used an RTX 4080 SUPER (16 GiB, driver 621.14), a 3440 × 1440 display and a generated 1280 × 720 H.264 motion pattern with audio at nominal 24 fps. These measurements apply to that source, GPU and display. The fullscreen 16:9 image occupied 2560 × 1440 pixels.

## Displayed frames

Intel PresentMon 2.6.0 measured the native video swap chain. Median and percentile intervals include the effects of the display refresh cadence. Other desktop work continued during the captures.

| SDR mode at 1920 × 1080 | Duration | Displayed fps | Median interval | p95 | p99 | Unshown presents |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Original rate | 7.92 s | 24.00 | — | 42.46 ms | 48.53 ms | 0 |
| RIFE v4.25-lite 2.5× and Anime4K | 604.97 s | 59.82 | 18.18 ms | 18.24 ms | 18.42 ms | 0 |

The enhanced run recorded 36,186 displayed intervals. Median GPU busy time was 9.15 ms and p95 was 15.24 ms. Whole-device utilization averaged 54% and peaked at 66%. Peak device memory usage was 5741 MiB, peak temperature was 72.32 °C and peak power was 138.09 W. The player reported five dropped frames and an A/V offset of 0 ms at the final sample. A separate ten-minute baseline capture crossed end-of-file and a seek, with a 7.25 s gap.

## Higher processing resolutions

These ten-second samples counted changes in the downsampled luma of a central desktop region. Desktop Duplication sampled the generated pattern after each live reconfiguration. Startup and previous configurations can affect these short samples. The paused control recorded zero changes in 2.99 s and the baseline playback control recorded 120 changes in 4.99 s.

| Processing and requested rate | Visible video area | Changes/s | Final absolute A/V offset |
| --- | ---: | ---: | ---: |
| Original, 24 fps | 1920 × 1080 | 24.11 | 0 ms |
| VSR 1920 × 1080, 24 fps | 1920 × 1080 | 24.01 | 0 ms |
| RIFE 2.5× alone, 60 fps | 1920 × 1080 | 37.22 | 0 ms |
| VSR 1920 × 1080 and RIFE 2.5× | 1920 × 1080 | 58.03 | 0 ms |
| VSR 2560 × 1440 and RIFE 5× | 2560 × 1440 | 8.21 | 5514 ms |
| VSR 3840 × 2160 and RIFE 2.5× | 2560 × 1440 | 8.31 | 1713 ms |
| VSR 3840 × 2160 and RIFE 5× | 2560 × 1440 | 8.61 | 6367 ms |
| VSR 3840 × 2160 and RIFE 6× | 2560 × 1440 | 9.11 | 13187 ms |

In a separate run, adaptive mode observed 63.4 drops/s and 1089 ms of A/V offset at 6×. It disabled interpolation. The next sample showed hardware decoding, the original frame rate and 0 ms of A/V offset.

## Image and HDR checks

- A paused Anime4K comparison changed 635 sampled pixels. Disabling the shader restored the original sample. Separate RIFE checks measured intermediate object positions and timestamps across a moving pattern and a scene cut.
- The NVIDIA driver accepted the VSR extension and produced the requested dimensions. Two fixed-frame comparisons against conventional D3D11 scaling produced identical sampled images. A visual quality gain from VSR has yet to be demonstrated on this configuration.
- Native HDR10, RTX SDR-to-HDR conversion and combined HDR, VSR and interpolation each produced moving color on the desktop. In the three-effect check the renderer reported a 2560 × 1440 PQ signal, a BT.2020/PQ 10-bit swap chain and a 1499-nit target. The Windows display link reported 8 bits per channel. Physical 10-bit output and native UHD presentation require separate verification.
- No other GPU generation has been tested.

## Decoder checks

The player displayed moving color at 392 screen sample points on each generated clip below. The native video area was 942 × 530. Each original-rate sample reported D3D11VA, zero drops and 0 ms of A/V offset. These are short functional checks.

| Input | Source rate | Original-rate filter output |
| --- | ---: | ---: |
| VP9, 1280 × 720 | 24 fps | 24.00 fps |
| AV1, 1920 × 1080 | 30 fps | 30.00 fps |
| HEVC, 3840 × 2160 | 60 fps | 60.00 fps |

The AV1 input also produced a visible image with the NVIDIA filter processing at 3840 × 2160.

Use `scripts/presentmon_summary.py` to analyze a PresentMon trace and `tests/visible_cadence.py` to repeat the short screen-content measurement with a test-owned window.
