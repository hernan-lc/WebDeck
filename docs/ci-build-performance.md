# CI correctness and linker measurements

Normal pushes and pull requests use the platform's default linker. First verify
those jobs, including the plugin integration test and Linux portable rollback.
Example plugin sources are checked against their manifest SHA-256 digests after
checkout; `.gitattributes` preserves LF even with `core.autocrlf=true`.

Dispatch `ci` on the same revision with `fast_linkers` disabled and enabled to
compare linkers. Enabled runs attempt clang/mold on Linux and detect
`lld-link.exe` on Windows, falling back to the default linker when unavailable.
The selected linker stays fixed throughout the job and has a separate Rust cache
key. Developers do not need to install these tools.

Rust tests, native host builds, and packaging Cargo invocations emit `--timings`
reports. Download `cargo-timings-linux` and `cargo-timings-windows` and compare
them alongside Actions step durations. Packaging step durations include archive
creation; Cargo reports only measure compilation. Repeat both configurations to
measure restored-cache behavior. For cold comparisons, remove the corresponding
Actions caches before each run. Comparing a new linker key against a warm default
cache is not a fair comparison. Record runner image, Rust version, revision,
selected linker, cache state, build/test/package durations, and total job duration.

All existing Clippy, integration, portable, installer, and application checks stay
enabled. Promote a linker to the default after these checks pass and comparable
cold and warm measurements show a useful improvement. Windows compatibility needs
native runner results. The configuration alone does not establish a speedup.

The explicit trusted-plugin example build remains necessary: `cargo test
--all-targets` builds test harnesses rather than the standalone executable used
by the ignored crash-isolation test. Packaging also needs normal application
executables. Measure these operations before removing them.

The production updater endpoint uses `nglmercer/WebDeck`; its release-selection
test matches that policy. Switching to `hernan-lc` requires a coordinated endpoint
and release-policy change. Duplicate `napi-vm-plugin-sdk` and protocol discovery
warnings come from vendored CLI templates in the pinned dependency. Fix them
upstream or in a reviewed dependency revision rather than Cargo's checkout cache.
