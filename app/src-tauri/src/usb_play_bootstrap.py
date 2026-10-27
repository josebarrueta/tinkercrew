"""Fixed physical-USB bootstrap. No passwords, permanent Pairings, SSH edits or arbitrary IPC.

The native bundle replaces EXPECTED_FILES with its exact six runtime digests.
The constructor is the fixed-helper fixture seam; main never accepts a root or code.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import ssl
import stat
import sys
import time
import uuid

ROOT = Path('/home/arduino/ArduinoApps/tinkercrew-companion')
EXPECTED_FILES = None  # Replaced by the compiled native bundle, never IPC input.


class BootstrapError(Exception):
    def __init__(self, code):
        self.code = code


def read_file(path, maximum, *, private=False):
    path = Path(path)
    if any(part.is_symlink() for part in (path, *path.parents)):
        raise BootstrapError('unsupported_storage')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as handle:
        info = os.fstat(handle.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_uid != os.geteuid() or info.st_mode & (0o077 if private else 0o022):
            raise BootstrapError('unsupported_storage')
        data = handle.read(maximum + 1)
        if len(data) > maximum:
            raise BootstrapError('unsupported_storage')
        return data


def host_identity():
    compatible = Path('/proc/device-tree/compatible').read_bytes().split(b'\0')
    os_release = dict(line.split('=', 1) for line in Path('/etc/os-release').read_text().splitlines() if '=' in line)
    if b'arduino,imola' not in compatible or os_release.get('ID', '').strip('"') != 'debian':
        raise BootstrapError('unsupported_board')
    raw_serial = Path('/sys/devices/soc0/serial_number').read_text().strip()
    try:
        serial = int(raw_serial, 16 if raw_serial.lower().startswith('0x') else 10)
    except ValueError:
        raise BootstrapError('unsupported_board') from None
    if not 0 < serial < 2**63:
        raise BootstrapError('unsupported_board')
    return {'serial':str(serial), 'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip()}


class UsbPlayBootstrap:
    def __init__(self, root, expected_files, *, inspect=host_identity):
        self.root, self.expected_files, self.inspect = Path(root), expected_files, inspect

    def dispatch(self, request):
        if not isinstance(request, dict) or request.get('operation') not in {'begin','refresh'}:
            raise BootstrapError('unsupported_operation')
        fields = {'operation','expected_serial'} | ({'board_id','boot_id','session_token'} if request['operation'] == 'refresh' else set())
        if set(request) != fields:
            raise BootstrapError('invalid_message')
        identity = self.inspect()
        if request['expected_serial'] != identity['serial']:
            raise BootstrapError('identity_mismatch')
        try:
            config = json.loads(read_file(self.root/'data/companion.json',8192,private=True))
        except FileNotFoundError:
            raise BootstrapError('usb_play_setup_required') from None
        if config.get('auth_mode') != 'tbp' or config.get('api_token') or config.get('port',7443) != 7443:
            raise BootstrapError('usb_play_setup_required')
        board = config.get('board_id')
        if not isinstance(board,str) or str(uuid.UUID(board)) != board:
            raise BootstrapError('unsupported_configuration')
        if request['operation'] == 'refresh' and (request['board_id'] != board or request['boot_id'] != identity['boot_id']):
            raise BootstrapError('identity_mismatch')
        tls_name = config.get('tls_name','')
        if not isinstance(tls_name,str) or len(tls_name)>253 or not re.fullmatch(r'[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+',tls_name):
            raise BootstrapError('unsupported_configuration')
        # Read only the fixed public certificate; never follow a config-provided path.
        certificate = read_file(self.root/'data/board.crt',8192).decode('ascii')
        if certificate.count('-----BEGIN CERTIFICATE-----') != 1 or 'PRIVATE KEY' in certificate:
            raise BootstrapError('unsupported_configuration')
        certificate_sha = hashlib.sha256(ssl.PEM_cert_to_DER_cert(certificate)).hexdigest()
        location = self.root
        runtime = self.root/'data/runtime'
        selection_path = runtime/'selection.json'
        if selection_path.exists() or selection_path.is_symlink():
            selected = json.loads(read_file(selection_path,8192,private=True))
            # Trial health may be authenticated but gameplay remains blocked by the
            # existing server maintenance gate. Bootstrap cannot promote a runtime.
            current = selected.get('current','')
            if not isinstance(current,str) or not re.fullmatch(r'[a-f0-9]{64}',current):
                raise BootstrapError('unsupported_storage')
            slot = runtime/'releases'/current
            if slot.exists() or slot.is_symlink():
                location = slot
        if not isinstance(self.expected_files,dict) or len(self.expected_files)!=6:
            raise BootstrapError('usb_play_upgrade_required')
        for name, expected in self.expected_files.items():
            if name not in {'python/tinkercrew_companion/'+x for x in ('__init__.py','auth.py','core.py','discovery.py','hardware.py','server.py')} or hashlib.sha256(read_file(location/name,2*1024*1024)).hexdigest()!=expected:
                raise BootstrapError('usb_play_upgrade_required')
        # Import only the digest-checked package, under an isolated module name.
        # -B on the native command prevents bytecode/persistent credential artifacts.
        package = location/'python/tinkercrew_companion'
        alias = '_tc_usb_runtime_'+self.expected_files['python/tinkercrew_companion/server.py']
        if alias not in sys.modules:
            spec = importlib.util.spec_from_file_location(alias,package/'__init__.py',submodule_search_locations=[str(package)])
            module = importlib.util.module_from_spec(spec);sys.modules[alias]=module;spec.loader.exec_module(module)
        server = __import__(alias+'.server',fromlist=['UsbPlayRegistry'])
        registry = server.UsbPlayRegistry(self.root/'data',board,boot_id=identity['boot_id'])
        metadata={'usb_play_version':1,'board_id':board,'boot_id':identity['boot_id'],'serial':identity['serial'],'tls_name':tls_name,'certificate_sha256':certificate_sha}
        if request['operation']=='begin':
            metadata.update(registry.begin())
            metadata['certificate_pem']=certificate
        else:
            metadata.update(registry.refresh(request['session_token']))
        return metadata


def main():
    try:
        raw=sys.stdin.buffer.read(2049)
        if len(raw)>2048:
            raise BootstrapError('message_too_large')
        result=UsbPlayBootstrap(ROOT,EXPECTED_FILES).dispatch(json.loads(raw))
        print(json.dumps({'status':'ok','result':result},separators=(',',':')))
    except Exception as error:
        # Never reflect exceptions/input/source/config; they may contain credentials.
        code=getattr(error,'code',None)
        allowed={'unsupported_storage','unsupported_operation','invalid_message','identity_mismatch','unsupported_board','unsupported_configuration','usb_play_setup_required','usb_play_upgrade_required','usb_play_unavailable','usb_session_expired','session_limit','message_too_large','authentication_required'}
        print(json.dumps({'status':'error','error':{'code':code if code in allowed else 'usb_play_unavailable'}}))


if __name__=='__main__':
    main()
