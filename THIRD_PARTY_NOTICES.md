# Third-party components

The Windows runtime manifest identifies a community build distributed by shinchiro and listed on the mpv project's Windows installation page. The download script verifies hashes. The repository contains no copy of the runtime. Review the build recipe, corresponding source and component licenses before redistributing a bundle. [Windows builds](https://mpv.io/installation/) · [Build source](https://github.com/shinchiro/mpv-winbuild-cmake) · [mpv copyright and license options](https://github.com/mpv-player/mpv/blob/bcde6b93cc/Copyright)

Rust crate versions and registry checksums are recorded in Cargo.lock. libloading is provided under the ISC license. windows-link is provided under MIT OR Apache-2.0. Cargo resolves additional target-specific dependencies for other platforms. See each crate's distribution for its license texts.

RTX Player source is licensed under GPL-3.0-or-later.
