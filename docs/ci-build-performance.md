# CI correctness and linker measurements

Linux pushes and pull requests use the default linker. Windows detects LLVM
`lld-link` (on PATH or in `Program Files/LLVM/bin`) and falls back to MSVC when
unavailable. Example plugin sources are checked against their manifest SHA-256
digests after checkout; `.gitattributes` preserves LF with `core.autocrlf=true`.

Windows CI uses `line-tables-only` debug information for development and test
profiles, retaining line-level backtraces while reducing object and PDB size.
Release profiles and developer builds are unchanged. Its Rust cache is saved even
when a later test fails, so a failing assertion does not discard compiled
dependencies. Cache keys separate these profile settings and the selected linker.

Development portable ZIPs use deflate level 1; Windows development installers use
`lzma2/fast`. These favor packaging time over artifact size. Release compression
stays unchanged, and checksums, payload checks, and installed-application checks
still run. Compare packaging and upload durations together because larger
development artifacts may take longer to transfer.

Validation hosts use shared cleanup that recognizes both ordinary and signal
exits, subscribes before termination, and bounds graceful and forced shutdown.
Process-cleanup regression tests run in both native CI jobs.

Dispatch `ci` on the same revision with `fast_linkers` disabled and enabled to
compare Linux's default linker against clang/mold. Set `windows_lld` false and
true to compare MSVC against detected `lld-link`. Both tools have availability
fallbacks and remain fixed throughout the job. Developers do not need them.

Rust tests, native host builds, and packaging Cargo invocations emit `--timings`
reports. Download `cargo-timings-linux` and `cargo-timings-windows` and compare
them alongside Actions step durations. Packaging step durations include archive
creation; Cargo reports only measure compilation. Repeat both configurations to
measure restored-cache behavior. For cold comparisons, remove the corresponding
Actions caches before each run. Comparing a new linker key against a warm default
cache is not a fair comparison. Record runner image, Rust version, revision,
selected linker, cache state, build/test/package durations, and total job duration.

All existing Clippy, integration, portable, installer, and application checks stay
enabled. Compare cold and warm measurements before claiming a speedup. Windows linker
compatibility and performance need native runner results; the configuration alone
does not establish a speedup.

The explicit trusted-plugin example build remains necessary: `cargo test
--all-targets` builds test harnesses rather than the standalone executable used
by the ignored crash-isolation test. Packaging also needs normal application
executables. Measure these operations before removing them.

The production updater endpoint uses `nglmercer/WebDeck`; its release-selection
test matches that policy. Switching to `hernan-lc` requires a coordinated endpoint
and release-policy change. Duplicate `napi-vm-plugin-sdk` and protocol discovery
warnings come from vendored CLI templates in the pinned dependency. Fix them
upstream or in a reviewed dependency revision rather than Cargo's checkout cache.
