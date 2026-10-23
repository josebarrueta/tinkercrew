"""Optional Bonjour announcement; never a trust or authorization mechanism."""
import ipaddress
import json
import os
import socket


def advertise(board_id: str, port: int, hostname: str):
    if board_id == "unprovisioned":
        return None
    try:
        from zeroconf import ServiceInfo, Zeroconf

        addresses = set()
        configured = os.environ.get("TC_ADVERTISE_IPV4")
        if configured:
            addresses.add(configured)
        else:
            for result in socket.getaddrinfo(socket.gethostname(), None, socket.AF_INET):
                addresses.add(result[4][0])
            # UDP connect selects the local routed interface without transmitting a packet.
            try:
                with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as route:
                    route.connect(("192.0.2.1", 9))
                    addresses.add(route.getsockname()[0])
            except OSError:
                pass
        packed = [socket.inet_aton(address) for address in addresses
                  if ipaddress.ip_address(address).is_private and not ipaddress.ip_address(address).is_loopback]
        if not packed:
            return None
        host = (hostname or socket.gethostname() + ".local").rstrip(".") + "."
        info = ServiceInfo("_tinkercrew._tcp.local.", f"{board_id}._tinkercrew._tcp.local.",
                           addresses=packed, port=port, properties={"board_id": board_id, "protocol": "1.0"}, server=host)
        zeroconf = Zeroconf()
        try:
            zeroconf.register_service(info)
        except Exception:
            zeroconf.close()
            raise
        return zeroconf
    except Exception:
        # Discovery must not prevent USB/manual gameplay or log credentials/raw exceptions.
        print(json.dumps({"event": "discovery_unavailable", "next_action": "Use USB or a manual local IP address."}))
        return None
