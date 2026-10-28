"""USB management grants + real pinned TLS, no hardware, Mac Keychain or real credentials."""
import datetime
import http.client
import json
from pathlib import Path
import socket
import ssl
import tempfile
import threading
import unittest
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID
from tinkercrew_companion.auth import PairingRegistry
from tinkercrew_companion.core import Companion
from tinkercrew_companion.hardware import MemoryHardware
from tinkercrew_companion.server import CompanionHTTPServer, CompanionUSBServer, UsbPlayRegistry, make_handler

BOARD='98f6411f-20ec-438a-a1de-6ce20fb0370e'

class UsbPlayTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(); self.root=Path(self.temp.name).resolve();self.now=1000.0;self.maintenance=False
        self.parent=PairingRegistry(self.root/'data',BOARD,boot_id='test-boot',clock=lambda:self.now)
        self.usb=UsbPlayRegistry(self.root/'data',BOARD,boot_id='test-boot',clock=lambda:self.now)
        self.companion=Companion(MemoryHardware(),board_id=BOARD,clock=lambda:self.now)
        handler=make_handler(self.companion,auth=self.parent,usb_play=self.usb,rate_clock=lambda:self.now,control_clock=lambda:self.now,maintenance=lambda:self.maintenance)
        self.server=CompanionHTTPServer(('0.0.0.0',0),handler)
        self.usb_server=CompanionUSBServer(str(self.root/'u.sock'),handler)
        key=rsa.generate_private_key(public_exponent=65537,key_size=2048)
        name=x509.Name([x509.NameAttribute(NameOID.COMMON_NAME,'localhost')]);now=datetime.datetime.now(datetime.timezone.utc)
        cert=x509.CertificateBuilder().subject_name(name).issuer_name(name).public_key(key.public_key()).serial_number(x509.random_serial_number()).not_valid_before(now-datetime.timedelta(minutes=1)).not_valid_after(now+datetime.timedelta(days=1)).add_extension(x509.SubjectAlternativeName([x509.DNSName('localhost')]),critical=False).sign(key,hashes.SHA256())
        self.cert=self.root/'cert.pem'; self.cert.write_bytes(cert.public_bytes(serialization.Encoding.PEM))
        private=self.root/'key.pem';private.write_bytes(key.private_bytes(serialization.Encoding.PEM,serialization.PrivateFormat.PKCS8,serialization.NoEncryption()))
        context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(self.cert,private);self.server.socket=context.wrap_socket(self.server.socket,server_side=True)
        self.usb_server.socket=context.wrap_socket(self.usb_server.socket,server_side=True)
        self.usb_thread=threading.Thread(target=self.usb_server.serve_forever,daemon=True);self.usb_thread.start()
        self.thread=threading.Thread(target=self.server.serve_forever,daemon=True);self.thread.start()
    def tearDown(self):
        self.server.shutdown();self.server.server_close();self.thread.join()
        self.usb_server.shutdown();self.usb_server.server_close();self.usb_thread.join();self.temp.cleanup()
    def request(self,path,body=None,token=None,method=None,peer=None,extra=None):
        if peer is None:
            raw=socket.socket(socket.AF_UNIX);raw.settimeout(2);raw.connect(str(self.root/'u.sock'))
        else:
            raw=socket.create_connection((peer,self.server.server_port),timeout=2)
        channel=ssl.create_default_context(cafile=str(self.cert)).wrap_socket(raw,server_hostname='localhost')
        connection=http.client.HTTPConnection('localhost',self.server.server_port);connection.sock=channel
        headers={'Content-Type':'application/json',**(extra or {})}
        if token:headers['Authorization']='Bearer '+token
        connection.request(method or ('POST' if body is not None else 'GET'),path,body=None if body is None else json.dumps(body),headers=headers)
        response=connection.getresponse();result=response.status,json.load(response);connection.close();return result
    def test_real_server_entrypoint_starts_private_tls_without_physical_hardware(self):
        import os
        import subprocess
        import sys
        import time
        config=self.root/'fixture.json';config.write_text(json.dumps({'auth_mode':'tbp','board_id':BOARD,'bind':'127.0.0.1','port':0,'tls_cert':str(self.cert),'tls_key':str(self.root/'key.pem')}));config.chmod(0o600)
        environment={key:value for key,value in os.environ.items() if not key.startswith('TC_')}
        environment.update(TC_CONFIG=str(config),TC_SIMULATE_HARDWARE='1',PYTHONPATH=str(Path(__file__).resolve().parents[1]/'python'),PYTHONDONTWRITEBYTECODE='1')
        child=subprocess.Popen([sys.executable,'-B','-m','tinkercrew_companion.server'],env=environment,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
        try:
            deadline=time.monotonic()+3
            path=self.root/'usb-play.sock'
            while not path.exists() and child.poll() is None and time.monotonic()<deadline:time.sleep(.02)
            self.assertIsNone(child.poll(),child.stderr.read().decode() if child.poll() is not None else 'private listener not ready')
            self.assertTrue(path.exists(),'Real server entrypoint did not create the private ingress')
            raw=socket.socket(socket.AF_UNIX);raw.settimeout(2);raw.connect(str(path))
            channel=ssl.create_default_context(cafile=str(self.cert)).wrap_socket(raw,server_hostname='localhost')
            connection=http.client.HTTPConnection('localhost');connection.sock=channel;connection.request('GET','/v1/health/public')
            response=connection.getresponse();self.assertEqual(response.status,200);self.assertEqual(json.load(response)['board_id'],BOARD);connection.close()
        finally:
            child.terminate()
            try:child.wait(timeout=3)
            except subprocess.TimeoutExpired:child.kill();child.wait()
            child.stderr.close()

    def test_delayed_bridge_samples_keep_receive_age_and_button_event_time(self):
        self.now+=3
        self.companion.record_distance(250,observed_at=1001.0)
        self.companion.record_buttons([False,False,False],observed_at=1001.0)
        self.companion.record_buttons([True,False,False],observed_at=1001.1)
        status,session=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200);token=session['session_token']
        status,state=self.request('/v1/state',token=token);self.assertEqual(status,200)
        self.assertEqual(state['observables']['distance']['health'],'stale')
        self.assertEqual(state['observables']['distance']['age_ms'],2000)
        events=self.request('/v1/events?after=0',token=token)[1]['events']
        edge=next(event for event in events if event['type']=='button.changed')
        self.assertAlmostEqual(edge['monotonic_ms'],1100,delta=1)

    def test_private_listener_never_replaces_a_live_socket_or_an_unknown_file(self):
        with self.assertRaises(ValueError):
            CompanionUSBServer(str(self.root/'u.sock'),self.server.RequestHandlerClass)
        unknown=self.root/'unknown.sock';unknown.write_text('keep')
        with self.assertRaises(ValueError):
            CompanionUSBServer(str(unknown),self.server.RequestHandlerClass)
        self.assertEqual(unknown.read_text(),'keep')
        status,result=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200)
        self.assertEqual(self.request('/v1/hello',token=result['session_token'])[0],200)

    def test_module_preview_can_clear_before_its_first_lease_without_losing_the_session(self):
        status,result=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200);token=result['session_token']
        # CreationRuntime.test/start first releases previous control. Opening a
        # real connection has deliberately not acquired any output lease.
        clear={'operation':'outputs.clear','arguments':{}}
        self.assertEqual(self.request('/v1/commands',clear,token)[0],200)
        self.assertEqual(self.request('/v1/hello',token=token)[1]['health']['safe_state'],True)
        self.assertEqual(self.request('/v1/control-lease',{'lease_ms':1500},token)[0],200)
        self.assertEqual(self.request('/v1/commands',{'operation':'pixels.set','arguments':{'color':'#3366ff','brightness':0.25,'duration_ms':1500}},token)[0],200)
        self.assertEqual(self.request('/v1/commands',clear,token)[0],200)
        self.assertEqual(self.request('/v1/commands',clear,token)[0],200)
        self.assertEqual(self.request('/v1/hello',token=token)[0],200)
        status,other=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200)
        self.assertEqual(self.request('/v1/control-lease',{'lease_ms':1500},other['session_token'])[0],200)

    def test_usb_session_cap_and_control_owner_are_independent_of_connect(self):
        tokens=[]
        for _ in range(2):
            status,result=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200);tokens.append(result['session_token'])
        self.assertEqual(self.request('/v1/usb-sessions',self.usb.begin())[0],429)
        self.assertEqual(self.request('/v1/hello',token=tokens[0])[1]['health']['safe_state'],True)
        self.assertEqual(self.request('/v1/control-lease',{'lease_ms':1500},tokens[0])[0],200)
        self.assertEqual(self.request('/v1/control-lease',{'lease_ms':1500},tokens[1])[0],409)
        self.assertEqual(self.request('/v1/commands',{'operation':'outputs.clear','arguments':{}},tokens[1])[0],409)
        self.assertEqual(self.request('/v1/sessions/current',token=tokens[0],method='DELETE')[0],200)
        self.assertEqual(self.request('/v1/hello',token=tokens[1])[1]['health']['safe_state'],True)
        self.assertEqual(self.request('/v1/usb-sessions',self.usb.begin())[0],200)

    def test_absolute_expiry_survives_active_reads_and_usb_proof_cannot_resurrect_idle_permission(self):
        from tinkercrew_companion.auth import AuthError
        status,result=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200);token=result['session_token']
        for elapsed in (50,50,49):
            self.now+=elapsed;self.assertEqual(self.request('/v1/hello',token=token)[0],200)
        self.now+=1;self.assertEqual(self.request('/v1/hello',token=token)[0],401)
        self.now+=1;status,result=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200);token=result['session_token']
        self.now+=61
        with self.assertRaises(AuthError):self.usb.refresh(token)
        self.assertEqual(self.request('/v1/state',token=token)[0],401)

    def test_maintenance_allows_authenticated_trial_health_but_never_a_usb_output_lease(self):
        self.maintenance=True
        status,result=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200);token=result['session_token']
        self.assertEqual(self.request('/v1/hello',token=token)[1]['health']['safe_state'],True)
        self.assertEqual(self.request('/v1/control-lease',{'lease_ms':1500},token)[0],503)
        self.maintenance=False
        self.assertEqual(self.request('/v1/hello',token=token)[1]['health']['safe_state'],True)

    def test_native_fixed_bootstrap_uses_exact_runtime_and_rejects_drift_before_issuing_permission(self):
        import hashlib
        import importlib.util
        import shutil
        import time
        self.now=time.monotonic()
        repository=Path(__file__).resolve().parents[2]
        spec=importlib.util.spec_from_file_location('usb_bootstrap_fixture',repository/'app/src-tauri/src/usb_play_bootstrap.py')
        helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
        expected={}
        for name in ('__init__.py','auth.py','core.py','discovery.py','hardware.py','server.py'):
            relative='python/tinkercrew_companion/'+name
            target=self.root/relative;target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(repository/'companion'/relative,target)
            expected[relative]=hashlib.sha256(target.read_bytes()).hexdigest()
        (self.root/'data/board.crt').write_bytes(self.cert.read_bytes())
        config=self.root/'data/companion.json';config.write_text(json.dumps({'auth_mode':'tbp','board_id':BOARD,'tls_name':'usb-board.local'}));config.chmod(0o600)
        bootstrap=helper.UsbPlayBootstrap(self.root,expected,inspect=lambda:{'serial':'42','boot_id':'test-boot'})
        result=bootstrap.dispatch({'operation':'begin','expected_serial':'42'})
        body={key:result[key] for key in ('board_id','grant_id','grant_token')}
        self.assertEqual(self.request('/v1/usb-sessions',body)[0],200)
        self.assertEqual(self.parent.list_pairings(),[])
        target=self.root/'python/tinkercrew_companion/server.py';target.write_text(target.read_text()+'\n# changed\n')
        with self.assertRaises(helper.BootstrapError) as caught:
            bootstrap.dispatch({'operation':'begin','expected_serial':'42'})
        self.assertEqual(caught.exception.code,'usb_play_upgrade_required')

    def test_network_peer_cannot_exchange_or_use_usb_tokens_even_with_spoofed_headers(self):
        with socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as route:
            route.connect(('192.0.2.1',9))  # Route lookup only; never sends a packet.
            local=route.getsockname()[0]
        if local.startswith('127.'):
            self.skipTest('No non-loopback interface for the real peer check')
        grant=self.usb.begin()
        spoof={'X-Forwarded-For':'127.0.0.1','X-Real-IP':'127.0.0.1'}
        self.assertEqual(self.request('/v1/usb-sessions',grant,peer=local,extra=spoof)[0],401)
        self.assertEqual(self.request('/v1/usb-sessions',grant,peer='127.0.0.1',extra=spoof)[0],401)
        status,result=self.request('/v1/usb-sessions',grant)
        self.assertEqual(status,200)
        for path,body,method in [('/v1/hello',None,'GET'),('/v1/control-lease',{'lease_ms':1500},'POST'),('/v1/commands',{'operation':'pixels.set','arguments':{}},'POST'),('/v1/sessions/current',None,'DELETE')]:
            self.assertEqual(self.request(path,body,result['session_token'],method,peer=local,extra=spoof)[0],401)
        self.assertEqual(self.request('/v1/hello',token=result['session_token'])[0],200)

    def test_usb_reproof_is_not_http_renewal_and_expired_or_restarted_sessions_stay_dead(self):
        expired=self.usb.begin();self.now+=31
        self.assertEqual(self.request('/v1/usb-sessions',expired)[0],401)
        self.now+=1; status,result=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200)
        token=result['session_token'];self.now+=40
        self.assertEqual(self.request('/v1/hello',token=token)[0],200)
        self.usb.refresh(token)  # Fixed physical USB management operation, not a network route.
        self.now+=40; self.assertEqual(self.request('/v1/hello',token=token)[0],200)
        self.now+=151;self.assertEqual(self.request('/v1/hello',token=token)[0],401)
        self.now+=1;status,result=self.request('/v1/usb-sessions',self.usb.begin());self.assertEqual(status,200)
        self.usb.start_runtime()
        self.assertEqual(self.request('/v1/hello',token=result['session_token'])[0],401)

    def test_usb_grant_is_one_use_and_never_creates_installation_or_ssh_registration(self):
        grant=self.usb.begin()
        status,result=self.request('/v1/usb-sessions',grant)
        self.assertEqual(status,200)
        token=result['session_token']
        self.assertEqual(self.request('/v1/hello',token=token)[0],200)
        self.assertEqual(self.request('/v1/usb-sessions',grant)[0],401)
        self.assertEqual(self.parent.list_pairings(),[])
        self.assertEqual(self.request('/v1/sessions/current',token=token,method='DELETE')[0],200)
        self.assertEqual(self.request('/v1/state',token=token)[0],401)
