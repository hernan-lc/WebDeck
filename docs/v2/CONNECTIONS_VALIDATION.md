# Unified device connections validation

Validated on Linux, 2026-10-11, with the production frontend embedded in the Rust
server and Chromium running separate localhost host and LAN phone contexts.

| Check | Result |
| --- | --- |
| `node tools/contracts/generate.mjs --check` | Passed; canonical schema and generated Rust/TypeScript/validators match |
| `node tools/validation/v2-only.mjs` | Passed; zero migration guard violations |
| `node tools/validation/i18n.mjs` | Passed; 513 UI translation keys |
| `cargo test --locked --all-targets` | 142 passed; two existing environment-dependent tests ignored |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| `npm run check:components --prefix frontend` | Zero errors and warnings |
| `npm run typecheck --prefix frontend` | Passed |
| `npm test --prefix frontend` | 73 passed across 13 suites |
| `npm run test:acceptance --prefix frontend` | 80 passed; one device-profile-only test skipped |
| `npm run test:devices --prefix frontend -- --project=android-chromium --project=tablet-chromium` | 12 passed; includes the touch test skipped in desktop acceptance |
| `npm run format:check --prefix frontend` | Passed |
| `npm run build --prefix frontend` and `cargo build --bin webdeck` | Passed |
| `node tools/validation/performance-budget.mjs` | Passed; 294945 bytes raw / 89178 gzip |

The optional trusted-process fixture was built with
`cargo build --locked --example trusted-plugin` and executed separately with
`WEBDECK_TRUSTED_FIXTURE=target/debug/examples/trusted-plugin cargo test --locked
--test runtime trusted_process_fixture_verifies_integrity_and_survives_plugin_crash
-- --ignored`; it passed. The ignored native portable upgrade test requires a
`WEBDECK_PORTABLE_ARTIFACT` produced by a native package build and was not executed.

New connection acceptance scenarios cover QR approval without manual credentials,
rejection and replay protection, timeout cleanup, remembered trust after reload and
browser restart, actual host process restart, offline/reconnect, immediate
revocation, independently authenticated devices and presence, explicit permission
upgrade followed by a new action attempt, permission rejection/approval using only
HTTP polling, origin-change re-pairing, automatic outstanding QR refresh after a
port/address change, and portrait/landscape layouts. Existing security, editor,
plugin, backup, runtime and accessibility regressions remain in the suites.

The bundle ceiling changed from 275000/85000 to 300000/91000 bytes to account for
the measured connection UI, trust/permission management, generated contracts and
reconnection controller. Timing ceilings remain unchanged; see the measurement
policy in `tools/validation/performance-budgets.json`.

Physical Wi-Fi sleep/wake, DHCP interface removal, router isolation, firewall
rules, OS notification delivery, and native phone cameras need real devices and
networks. Browser tests emulate disconnection and verify actual listener-origin
changes; they do not prove reachability through every router/firewall. Built-in LAN
transport is HTTP with explicit trusted-network guidance. Scoped IPv6 link-local
URLs, mDNS and an encrypted public relay are not provided. HTTPS deployment must
preserve the local administrative boundary; see [CONNECTIONS.md](CONNECTIONS.md).
