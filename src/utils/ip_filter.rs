// SPDX-License-Identifier: GPL-3.0-only

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Returns true when the IP must not be queried via the open LiveServerInfo endpoint.
pub fn is_blocked_query_target(ip: IpAddr) -> bool {
	match ip {
		IpAddr::V4(ipv4) => is_private_ipv4(&ipv4),
		IpAddr::V6(ipv6) => is_private_ipv6(&ipv6),
	}
}

fn is_private_ipv4(ip: &Ipv4Addr) -> bool {
	let octets = ip.octets();

	// 10.0.0.0/8
	octets[0] == 10
		// 172.16.0.0/12
		|| (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31)
		// 192.168.0.0/16
		|| (octets[0] == 192 && octets[1] == 168)
		// 169.254.0.0/16 link-local
		|| (octets[0] == 169 && octets[1] == 254)
		// 127.0.0.0/8 loopback
		|| octets[0] == 127
		// 100.64.0.0/10 CGNAT
		|| (octets[0] == 100 && (octets[1] & 0b1100_0000) == 0b0100_0000)
		// 0.0.0.0/8
		|| octets[0] == 0
}

fn is_private_ipv6(ip: &Ipv6Addr) -> bool {
	let segments = ip.segments();

	if segments == [0, 0, 0, 0, 0, 0, 0, 1] {
		return true;
	}

	// fc00::/7 unique local
	if (segments[0] & 0xfe00) == 0xfc00 {
		return true;
	}

	// fe80::/10 link-local
	if (segments[0] & 0xffc0) == 0xfe80 {
		return true;
	}

	// ::ffff:0:0/96 IPv4-mapped
	if segments[0] == 0
		&& segments[1] == 0
		&& segments[2] == 0
		&& segments[3] == 0
		&& segments[4] == 0
		&& segments[5] == 0xffff
	{
		let ipv4 = Ipv4Addr::new(
			((segments[6] >> 8) & 0xff) as u8,
			(segments[6] & 0xff) as u8,
			((segments[7] >> 8) & 0xff) as u8,
			(segments[7] & 0xff) as u8,
		);
		return is_private_ipv4(&ipv4);
	}

	false
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::net::Ipv4Addr;

	#[test]
	fn blocks_loopback_and_rfc1918() {
		assert!(is_blocked_query_target(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
		assert!(is_blocked_query_target(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
		assert!(is_blocked_query_target(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
	}

	#[test]
	fn allows_public_ipv4() {
		assert!(!is_blocked_query_target(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
	}
}
