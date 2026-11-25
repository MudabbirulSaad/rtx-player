# Third-party components

The Windows runtime manifest identifies a community build distributed by shinchiro and listed on the mpv project's Windows installation page. The download script verifies hashes. The repository contains no copy of the runtime. Review the build recipe, corresponding source and component licenses before redistributing a bundle. [Windows builds](https://mpv.io/installation/) · [Build source](https://github.com/shinchiro/mpv-winbuild-cmake) · [mpv copyright and license options](https://github.com/mpv-player/mpv/blob/bcde6b93cc/Copyright)

Rust crate versions and registry checksums are recorded in Cargo.lock. libloading is provided under the ISC license. windows-link is provided under MIT OR Apache-2.0. Cargo resolves additional target-specific dependencies for other platforms. See each crate's distribution for its license texts.

RTX Player source is licensed under GPL-3.0-or-later.

The Anime4K small 2× CNN shader is copyright bloc97 and licensed under MIT. Its source and license are included under assets/Anime4K. [Upstream revision](https://github.com/bloc97/Anime4K/tree/7684e9586f8dcc738af08a1cdceb024cc184f426)

The optional RIFE setup fetches upstream components directly into the selected application build. The manifest pins the origin and SHA-256 of each download. CPython's embedded distribution includes its PSF license and notices. The VapourSynth wheel includes its LGPL-2.1-or-later license. The NCNN filter project supplies the unsigned Windows DLL and the model pair. Its repository license is MIT. The redistributability of the model weights and the complete native dependency chain requires separate review before including them in a public binary release. [CPython](https://www.python.org/downloads/release/python-31410/) · [VapourSynth](https://github.com/vapoursynth/vapoursynth/tree/R82) · [Filter and model source](https://github.com/styler00dollar/VapourSynth-RIFE-ncnn-Vulkan/tree/c3ec6aabc07c8fa37a4f58d7fed9e2ad1fc1b13f)
