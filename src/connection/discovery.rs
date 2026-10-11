use crate::contracts::ConnectionAddress;
use std::net::{IpAddr, SocketAddr, UdpSocket};

pub fn usable(address: IpAddr) -> bool {
    !address.is_loopback()
        && !address.is_unspecified()
        && !address.is_multicast()
        && match address {
            IpAddr::V4(a) => !a.is_link_local() && !a.is_broadcast(),
            IpAddr::V6(a) => !a.is_unicast_link_local(),
        }
}
fn rank(name: &str, address: IpAddr, route: Option<IpAddr>) -> u8 {
    let name = name.to_ascii_lowercase();
    if [
        "docker",
        "veth",
        "virbr",
        "vmnet",
        "tun",
        "tap",
        "tailscale",
        "wg",
        "vpn",
        "utun",
    ]
    .iter()
    .any(|prefix| name.contains(prefix))
    {
        return 4;
    }
    if Some(address) == route {
        return 0;
    }
    if address.is_ipv4() {
        1
    } else {
        2
    }
}
pub fn addresses() -> Vec<ConnectionAddress> {
    let route = route_address();
    let networks = sysinfo::Networks::new_with_refreshed_list();
    let mut addresses: Vec<_> = networks
        .iter()
        .flat_map(|(name, data)| {
            data.ip_networks()
                .iter()
                .filter(|network| usable(network.addr))
                .map(|network| ConnectionAddress {
                    interface: name.clone(),
                    address: network.addr.to_string(),
                    preferred: false,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    addresses.sort_by_key(|entry| {
        (
            rank(
                &entry.interface,
                entry.address.parse().expect("interface IP"),
                route,
            ),
            entry.interface.clone(),
            entry.address.clone(),
        )
    });
    addresses.dedup_by(|a, b| a.address == b.address);
    if let Some(first) = addresses.first_mut() {
        first.preferred = true;
    }
    addresses
}
fn route_address() -> Option<IpAddr> {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("192.0.2.1:80")?;
            socket.local_addr()
        })
        .ok()
        .map(|a: SocketAddr| a.ip())
        .filter(|a| usable(*a))
}
pub fn suggested_address() -> String {
    addresses()
        .first()
        .map(|a| a.address.clone())
        .unwrap_or_default()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_unusable_and_prefers_physical_interfaces() {
        for ip in [
            "127.0.0.1",
            "0.0.0.0",
            "169.254.1.2",
            "224.0.0.1",
            "::1",
            "fe80::1",
        ] {
            assert!(!usable(ip.parse().unwrap()));
        }
        let ip = "192.168.1.2".parse().unwrap();
        assert!(usable(ip));
        assert!(rank("wlan0", ip, None) < rank("tun0", ip, Some(ip)));
        assert!(usable("fd00::1".parse().unwrap()));
    }
}
