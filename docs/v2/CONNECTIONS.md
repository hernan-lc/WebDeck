# Devices and connections

The host opens Connect Device, the wizard starts/reuses LAN access, and the phone
scans an invitation QR. The phone sends its suggested name automatically, compares
a six-digit verification code, and waits for a localhost administrator. Only the
claim secret and matching source address can receive the approved credential.
Invitation possession alone grants no action access.

## Ownership and security

`connection::manager::Network` owns the sole secondary LAN listener. `network` is
a compatibility facade. `Sessions` remains the device/grant/credential authority;
the connection manager never replaces the executor. HTTP and Socket.IO share the
same sessions. The primary localhost listener stays available across secondary
listener changes. Binds and atomic persistence finish before replacing the old
listener. Stop is idempotent; graceful shutdown has a bounded drain period.

Discovery enumerates IPv4 and IPv6 using the existing sysinfo dependency. Physical
interfaces precede VPN/virtual adapters, and default-route affinity breaks ties.
Loopback, unspecified, multicast, IPv4 link-local, and scoped IPv6 link-local
addresses are excluded. Automatic exposure chooses private IPv4/unique-local IPv6;
manual configuration remains available. This architecture binds one selected
address at a time. Rediscovery checks automatically started listeners every five
seconds during the process lifetime. Explicit advanced configuration disables
automatic reconfiguration; saved settings remain compatible with older builds.

Pairing resources are bounded: 32 invitations, 32 pending requests, two requests
per source, a two-minute lifetime, request/claim rate limits, and one winner for
concurrent invitation consumption and credential claims. Credentials are generated
with existing random IDs, stored as SHA-256 hashes, and delivered only through the
secret-authenticated claim endpoint. Ordinary realtime events contain no secrets.
Local-only approval, Host/Origin checks, allowed-network checks, and per-command
capability checks remain mandatory. Revocation blocks the next HTTP or realtime
command even if a socket has not disconnected yet. Realtime revocation events
close active sockets; a five-second policy heartbeat also detects expiry and
changes made by other processes.

HTTP LAN transport remains suitable only for trusted networks. Configure HTTPS
through a separately secured deployment if encryption is required. A proxy must
preserve peer identity and cannot forward external clients to the local admin
boundary as loopback. No internet relay or automatic public port forwarding is
provided. mDNS is not required; numeric IP URLs remain authoritative.

## Permissions and trust

Profiles preserve capability-based authorization. View Only grants `read`.
Control My Deck collects configured action requirements, nested workflow commands,
button references, script nodes, and plugin manifest action requirements. Full
Controller offers remotely grantable read/input/audio/window/power/script/network/
plugin access and requires explicit host confirmation. Custom exposes individual
permissions. Recommended profiles never modify an existing grant automatically.

Host approval selects 15-minute temporary trust, 30 days, or until revoked.
Until revoked uses the exact JSON-safe expiry sentinel 9007199254740991; it has no
scheduled expiry. Renew changes expiry only. Permission-upgrade approval merges
explicitly requested permissions into the current persisted grant, preserving
expiry; rejected, expired, or revoked requests cannot expand a grant. Devices must
retry their intended action themselves. No command is replayed by recovery.

## API additions

All management endpoints require the local administrator; device-self endpoints
require a valid device credential. Credential failures retain HTTP 401 and distinguish `authorization_expired`, `device_revoked`, and unknown `unauthorized` credentials. Existing `/network`, `/devices`, and `/pairing`
endpoints remain available.

| Method | `/api/v2` path | Result |
| --- | --- | --- |
| GET | `/connection/status`, `/connection/diagnostics` | `ConnectionStatus` |
| POST | `/connection/start` | `ConnectionStatus` |
| POST | `/connection/invitations` | `ConnectionInvitation` |
| GET / DELETE | `/connection/invitations/{id}` | `ConnectionProgress` / cancel |
| GET | `/connection/invitations/{id}/qr` | no-store PNG |
| GET | `/connection/profiles` | `PermissionProfiles` |
| GET | `/devices/presence` | `DevicePresenceList` |
| GET | `/device/self` | current `Device`, no credential |
| PATCH | `/devices/{id}` | `DeviceUpdate` → `Device` |
| POST | `/devices/{id}/permissions` | explicit `DeviceUpdate` → `Device` |
| POST | `/devices/{id}/renew` | `DeviceRenew` → `Device` |
| POST | `/device/self/permissions` | `PermissionRequest` → `PermissionPending` |
| GET | `/device/self/permissions/{id}` | owning device only, `ConnectionProgress` |
| GET | `/connection/permissions` | `PermissionRequests` |
| POST / DELETE | `/connection/permissions/{id}` | approve / reject |

`PairingRequest` accepts an optional invitation. Invitation URLs use a fragment,
which is removed by the phone before sending the request. QR refresh creates a new
short-lived invitation. Address changes automatically replace an outstanding QR invitation; permission decisions can be polled securely by their requesting device. The host progress endpoint never returns a claim secret
or credential. Claim delivery is single-use, including for legacy pairing requests.

Realtime events include `connection.status`, `device.connected`,
`device.disconnected`, `pairing.requested`, `pairing.approved`, `pairing.rejected`,
`device.permission_requested`, `device.permissions_changed`, and `device.revoked`.
Desktop pairing notifications are optional under Advanced and use browser permission; clicking a notification focuses the trusted host interface. Host events are local-only; device events go only to the matching authenticated
device or localhost. Initial synchronization and critical decisions use HTTP;
periodic HTTP polling remains available when realtime is unavailable.

## Reconnection and storage compatibility

One browser controller and one Socket.IO client handle recovery. Authorization
reads retry with bounded exponential backoff and jitter; Socket.IO reconnects
subscriptions. Visibility/online changes prompt a policy refresh. Expired or
revoked access requires approval; ordinary disconnects do not require pairing.
Unknown outcomes of command execution are never automatically retried.

`devices.v2.json` retains its existing array shape and hashed credentials.
`network.v2.json` retains its existing shape. Existing grants/tokens keep their
expiry and permissions. A new optional `connection-history.v2.json` sidecar stores
version 1 last-seen timestamps with atomic writes. Presence is ephemeral and is
always offline after process restart until a device authenticates again.

For rollback, stop the process and restore the old executable/frontend together.
Older builds ignore the history sidecar; retain it for future upgrades. No paired
device data conversion is required. Long-lived grants remain representable in the
old storage shape, though older interfaces cannot create them. Backup restores do
not import listener settings, device credentials, or connection history.

Changing the hostname/IP/port changes the browser origin. Saved access cannot be
transferred automatically: the supported recovery is a fresh invitation and host
approval. An unavailable phone can indicate firewall rules, separate networks,
client isolation, a VPN route, or an outdated QR; server binding alone cannot
identify which external network condition is responsible.

Validation evidence and platform limits: [CONNECTIONS_VALIDATION.md](CONNECTIONS_VALIDATION.md).
