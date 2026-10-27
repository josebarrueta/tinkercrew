"""Real TLS over private Unix ingress, with a loopback byte-forwarder standing in for ADB."""
import json
from pathlib import Path
import select
import socket
import socketserver
import ssl
import sys
import threading
from tinkercrew_companion.auth import PairingRegistry
from tinkercrew_companion.core import Companion
from tinkercrew_companion.hardware import MemoryHardware
from tinkercrew_companion.server import CompanionHTTPServer, CompanionUSBServer, UsbPlayRegistry, make_handler

board='98f6411f-20ec-438a-a1de-6ce20fb0370e'
root=Path(sys.argv[3]).resolve();root.chmod(0o700);directory=root/'data'
parent=PairingRegistry(directory,board);usb=UsbPlayRegistry(directory,board)
handler=make_handler(Companion(MemoryHardware(),board_id=board),auth=parent,usb_play=usb)
server=CompanionHTTPServer(('127.0.0.1',0),handler)
private=CompanionUSBServer(str(root/'u.sock'),handler)
context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(sys.argv[1],sys.argv[2])
server.socket=context.wrap_socket(server.socket,server_side=True)
private.socket=context.wrap_socket(private.socket,server_side=True)
class Forward(socketserver.BaseRequestHandler):
    def handle(self):
        with socket.socket(socket.AF_UNIX) as remote:
            remote.connect(str(root/'u.sock'))
            while True:
                ready,_,_=select.select([self.request,remote],[],[],3)
                if not ready:return
                for source in ready:
                    data=source.recv(65536)
                    if not data:return
                    (remote if source is self.request else self.request).sendall(data)
forward=socketserver.ThreadingTCPServer(('127.0.0.1',0),Forward)
for peer in (private,forward):threading.Thread(target=peer.serve_forever,daemon=True).start()
print(json.dumps({'port':forward.server_address[1],'boot_id':usb.boot_id,'grant':usb.begin()}),flush=True)
server.serve_forever()
