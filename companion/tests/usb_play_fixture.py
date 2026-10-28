"""Real TLS over private Unix ingress, with a loopback byte-forwarder standing in for ADB."""
import json
from pathlib import Path
import select
import socket
import socketserver
import ssl
import sys
import threading
import queue
import types
from tinkercrew_companion.auth import PairingRegistry
from tinkercrew_companion.core import Companion
from tinkercrew_companion.hardware import MemoryHardware
from tinkercrew_companion.server import build_companion
from tinkercrew_companion.server import CompanionHTTPServer, CompanionUSBServer, UsbPlayRegistry, make_handler

board='98f6411f-20ec-438a-a1de-6ce20fb0370e'
root=Path(sys.argv[3]).resolve();root.chmod(0o700);directory=root/'data'
parent=PairingRegistry(directory,board);usb=UsbPlayRegistry(directory,board)
if len(sys.argv)>4 and sys.argv[4]=='bridge-notify-before-ack':
    # External SDK seam: its single receiver delivers an input notification
    # before the reply to an outgoing RPC. No real Arduino SDK or hardware.
    class ReceiverBridge:
        def __init__(self):
            self.handlers={};self.calls=queue.Queue()
            threading.Thread(target=self.receive,daemon=True).start()
        def provide(self,name,handler):self.handlers[name]=handler
        def call(self,name,*args):
            replied=threading.Event();self.calls.put(replied)
            if not replied.wait(1.0):raise TimeoutError('Fixture receiver blocked before RPC acknowledgement')
            return True
        def receive(self):
            while True:
                replied=self.calls.get()
                self.handlers['tc_disconnected']('distance')
                replied.set()
    for name in ('arduino','arduino.app_utils'):
        module=types.ModuleType(name);module.__path__=[];sys.modules[name]=module
    module=types.ModuleType('arduino.app_utils.bridge');module.Bridge=ReceiverBridge();sys.modules[module.__name__]=module
    companion=build_companion(board_id=board)
else:
    companion=Companion(MemoryHardware(),board_id=board)
handler=make_handler(companion,auth=parent,usb_play=usb)
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
