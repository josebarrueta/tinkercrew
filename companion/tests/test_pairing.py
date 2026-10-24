"""Public HTTP + USB-management registry tests; temporary files and real loopback TLS only."""
import base64
import datetime
import http.client
import json
import pathlib
import ssl
import tempfile
import threading
import unittest

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ed25519, rsa
from cryptography.x509.oid import NameOID

from tinkercrew_companion.auth import AuthError, PairingRegistry
from tinkercrew_companion.core import Companion
from tinkercrew_companion.hardware import MemoryHardware
from tinkercrew_companion.server import CompanionHTTPServer, make_handler

BOARD = "98f6411f-20ec-438a-a1de-6ce20fb0370e"
INSTALLATION = "1f33fd77-7015-41c6-a882-650f37a06c18"


class PairingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temp.name)
        self.clock = 1000.0
        self.registry = PairingRegistry(self.root / "data", BOARD, ssh_directory=self.root / ".ssh", clock=lambda: self.clock, boot_id="test-boot")
        self.key = ed25519.Ed25519PrivateKey.generate()
        self.ssh_key = ed25519.Ed25519PrivateKey.generate()
        self.registration = {"installation_id": INSTALLATION, "label": "Test Mac", "public_key": base64.urlsafe_b64encode(self.key.public_key().public_bytes_raw()).decode().rstrip("="), "ssh_public_key": self.ssh_key.public_key().public_bytes(serialization.Encoding.OpenSSH, serialization.PublicFormat.OpenSSH).decode()}
        self.companion = Companion(MemoryHardware(), board_id=BOARD, clock=lambda: self.clock)
        self.companion.record_distance(250)
        self.server = CompanionHTTPServer(("127.0.0.1", 0), make_handler(self.companion, auth=self.registry, rate_clock=lambda: self.clock, control_clock=lambda: self.clock))
        cert_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "localhost")])
        now = datetime.datetime.now(datetime.timezone.utc)
        certificate = x509.CertificateBuilder().subject_name(name).issuer_name(name).public_key(cert_key.public_key()).serial_number(x509.random_serial_number()).not_valid_before(now - datetime.timedelta(minutes=1)).not_valid_after(now + datetime.timedelta(days=1)).add_extension(x509.SubjectAlternativeName([x509.DNSName("localhost")]), critical=False).sign(cert_key, hashes.SHA256())
        self.cert = self.root / "tls.crt"
        self.cert.write_bytes(certificate.public_bytes(serialization.Encoding.PEM))
        private = self.root / "tls.key"
        private.write_bytes(cert_key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(self.cert, private)
        self.server.socket = context.wrap_socket(self.server.socket, server_side=True)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        self.temp.cleanup()

    def request(self, path, body=None, token=None, method=None):
        context = ssl.create_default_context(cafile=str(self.cert))
        connection = http.client.HTTPSConnection("localhost", self.server.server_port, context=context, timeout=2)
        headers = {"Content-Type": "application/json"}
        if token:
            headers["Authorization"] = "Bearer " + token
        connection.request(method or ("POST" if body is not None else "GET"), path, body=None if body is None else json.dumps(body), headers=headers)
        response = connection.getresponse()
        result = response.status, json.load(response)
        connection.close()
        return result

    def pair(self):
        bootstrap = self.registry.begin_pairing(self.registration)
        payload = dict(self.registration, pairing_token=bootstrap["pairing_token"])
        status, result = self.request("/v1/pairings", payload)
        self.assertEqual(status, 200)
        self.assertEqual(result["state"], "pending_ssh")
        self.registry.activate_pairing(INSTALLATION)
        return payload

    def session(self, replacing=None):
        status, challenge = self.request("/v1/sessions/challenge", {"board_id": BOARD, "installation_id": INSTALLATION})
        self.assertEqual(status, 200)
        message = f'TBP1\n{BOARD}\n{INSTALLATION}\n{challenge["challenge_id"]}\n{challenge["nonce"]}\n{challenge["expires_at"]}\n'.encode()
        proof = {"board_id": BOARD, "installation_id": INSTALLATION, "challenge_id": challenge["challenge_id"], "signature": base64.urlsafe_b64encode(self.key.sign(message)).decode().rstrip("=")}
        status, result = self.request("/v1/sessions", proof, token=replacing)
        self.assertEqual(status, 200)
        return result["session_token"], proof

    def test_unauthenticated_proof_flood_does_not_consume_the_gameplay_command_budget(self):
        self.pair()
        token, _ = self.session()
        for _ in range(21):
            self.request("/v1/sessions", {})
        self.assertEqual(self.request("/v1/control-lease", {"lease_ms": 1500}, token)[0], 200)

    def test_malformed_unauthenticated_proof_cannot_clear_an_existing_control_lease(self):
        self.pair()
        token, _ = self.session()
        self.assertEqual(self.request("/v1/control-lease", {"lease_ms": 1500}, token)[0], 200)
        connection = http.client.HTTPSConnection("localhost", self.server.server_port, context=ssl.create_default_context(cafile=str(self.cert)), timeout=2)
        connection.request("POST", "/v1/sessions", body=b'{"installation_id":' + b'1' * 5000 + b'}')
        response = connection.getresponse()
        self.assertEqual(response.status, 400)
        response.read()
        connection.close()
        self.assertFalse(self.companion.state()["health"]["safe_state"])

    def test_revocation_denies_sessions_even_if_ssh_file_cleanup_fails(self):
        self.pair()
        token, _ = self.session()
        keys = self.root / ".ssh/authorized_keys"
        keys.unlink()
        unrelated = self.root / "unrelated.txt"
        unrelated.write_text("never overwrite this file")
        keys.symlink_to(unrelated)
        result = self.registry.revoke(INSTALLATION)
        self.assertTrue(result["revoked"])
        self.assertFalse(result["ssh_key_removed"])
        self.assertEqual(self.request("/v1/state", token=token)[0], 401)
        self.assertEqual(unrelated.read_text(), "never overwrite this file")
        self.assertFalse(self.registry.is_active(INSTALLATION))

    def test_tbp_mode_never_accepts_a_legacy_bearer_bypass(self):
        with self.assertRaises(ValueError):
            make_handler(self.companion, "legacy-token-" * 4, auth=self.registry)
        self.assertEqual(self.request("/v1/state", token="legacy-token-" * 4)[0], 401)
        self.assertEqual(self.request("/v1/pair_begin", {})[0], 401)

    def test_expired_challenge_rejects_even_a_correct_signature(self):
        self.pair()
        status, challenge = self.request("/v1/sessions/challenge", {"board_id": BOARD, "installation_id": INSTALLATION})
        self.assertEqual(status, 200)
        message = f'TBP1\n{BOARD}\n{INSTALLATION}\n{challenge["challenge_id"]}\n{challenge["nonce"]}\n{challenge["expires_at"]}\n'.encode()
        self.clock += 31
        self.assertEqual(self.request("/v1/sessions", {"board_id": BOARD, "installation_id": INSTALLATION, "challenge_id": challenge["challenge_id"], "signature": base64.urlsafe_b64encode(self.key.sign(message)).decode().rstrip("=")})[0], 401)

    def test_pairing_token_expiry_and_attempt_budget_do_not_grant_trust(self):
        bootstrap = self.registry.begin_pairing(self.registration)
        payload = dict(self.registration, pairing_token=bootstrap["pairing_token"])
        self.clock += 301
        self.assertEqual(self.request("/v1/pairings", payload)[0], 401)
        bootstrap = self.registry.begin_pairing(self.registration)
        for _ in range(5):
            self.assertEqual(self.request("/v1/pairings", dict(self.registration, pairing_token="z" * 43))[0], 401)
        self.assertEqual(self.request("/v1/pairings", dict(self.registration, pairing_token=bootstrap["pairing_token"]))[0], 401)
        self.assertEqual(self.registry.list_pairings(), [])

    def test_second_session_observes_but_cannot_steal_an_active_control_lease(self):
        self.pair()
        first, _ = self.session()
        second, _ = self.session()
        self.assertEqual(self.request("/v1/control-lease", {"lease_ms": 1500}, first)[0], 200)
        self.assertEqual(self.request("/v1/state", token=second)[0], 200)
        self.assertEqual(self.request("/v1/control-lease", {"lease_ms": 1500}, second)[0], 409)
        self.assertEqual(self.request("/v1/sessions/current", token=first, method="DELETE")[0], 200)
        self.assertEqual(self.request("/v1/control-lease", {"lease_ms": 1500}, second)[0], 200)
        self.assertEqual(self.request("/v1/state", token=first)[0], 401)

    def test_absolute_session_lifetime_and_logs_do_not_leak_credentials_or_query_secrets(self):
        import contextlib
        import io
        captured = io.StringIO()
        with contextlib.redirect_stdout(captured):
            payload = self.pair()
            token, proof = self.session()
            for _ in range(14):
                self.clock += 60
                self.assertEqual(self.request("/v1/state", token=token)[0], 200)
            self.clock += 60
            self.assertEqual(self.request("/v1/state", token=token)[0], 401)
            self.assertEqual(self.request("/v1/health/public?token=fixture-secret-query")[0], 200)
        for secret in [token, payload["pairing_token"], proof["signature"], "fixture-secret-query"]:
            self.assertNotIn(secret, captured.getvalue())
            for file in (self.root / "data").glob("*"):
                if file.is_file():
                    self.assertNotIn(secret.encode(), file.read_bytes())

    def test_signed_session_rotation_preserves_lease_but_invalidates_the_old_token(self):
        self.pair()
        old, _ = self.session()
        self.assertEqual(self.request("/v1/control-lease", {"lease_ms": 1500}, old)[0], 200)
        new, _ = self.session(replacing=old)
        self.assertNotEqual(old, new)
        self.assertEqual(self.request("/v1/state", token=old)[0], 401)
        self.assertEqual(self.request("/v1/control-lease", {"lease_ms": 1500}, new)[0], 200)
        self.assertFalse(self.companion.state()["health"]["safe_state"])

    def test_fixed_management_bootstrap_binds_board_and_never_exposes_a_shell(self):
        from tinkercrew_companion.management import PairingManagement
        config = self.root / "data/companion.json"
        config.write_text(json.dumps({"auth_mode": "tbp", "board_id": BOARD, "tls_name": "localhost", "tls_cert": str(self.cert)}))
        management = PairingManagement(self.root, ssh_directory=self.root / ".ssh", inspect=lambda: {"serial": "42", "username": "arduino"}, clock=lambda: self.clock, boot_id="test-boot")
        with self.assertRaises(AuthError) as wrong:
            management.dispatch({"operation": "pair_begin", "expected_serial": "43", "registration": self.registration})
        self.assertEqual(wrong.exception.code, "identity_mismatch")
        result = management.dispatch({"operation": "pair_begin", "expected_serial": "42", "registration": self.registration})
        self.assertEqual(result["board_id"], BOARD)
        self.assertEqual(result["tls_name"], "localhost")
        self.assertEqual(result["certificate_pem"], self.cert.read_text())
        self.assertEqual(self.request("/v1/pairings", dict(self.registration, pairing_token=result["pairing_token"]))[0], 200)
        self.assertEqual(management.dispatch({"operation": "pair_activate", "expected_serial": "42", "installation_id": INSTALLATION})["state"], "active")
        with self.assertRaises(AuthError) as unknown:
            management.dispatch({"operation": "shell", "command": "anything"})
        self.assertEqual(unknown.exception.code, "invalid_message")
        with self.assertRaises(AuthError) as forbidden:
            management.dispatch({"operation": "revoke", "expected_serial": "42", "installation_id": INSTALLATION}, ssh_installation=INSTALLATION)
        self.assertEqual(forbidden.exception.code, "authentication_failed")
        from unittest.mock import patch
        with patch("os.geteuid", return_value=0):
            with self.assertRaises(AuthError) as permissions:
                management.dispatch({"operation": "runtime_status", "expected_serial": "42"})
            self.assertEqual(permissions.exception.code, "unsupported_permissions")
        for operation, parameters in (("runtime_status", {}), ("runtime_cancel", {"release": "a" * 64})):
            with self.assertRaises(AuthError):
                management.dispatch(dict(operation=operation, expected_serial="42", **parameters), ssh_installation=INSTALLATION)

    def test_expired_and_invalid_proofs_are_single_attempt_and_sessions_expire(self):
        self.pair()
        status, challenge = self.request("/v1/sessions/challenge", {"board_id": BOARD, "installation_id": INSTALLATION})
        self.assertEqual(status, 200)
        message = f'TBP1\n{BOARD}\n{INSTALLATION}\n{challenge["challenge_id"]}\n{challenge["nonce"]}\n{challenge["expires_at"]}\n'.encode()
        good = base64.urlsafe_b64encode(self.key.sign(message)).decode().rstrip("=")
        proof = {"board_id": BOARD, "installation_id": INSTALLATION, "challenge_id": challenge["challenge_id"], "signature": base64.urlsafe_b64encode(b"x" * 64).decode().rstrip("=")}
        self.assertEqual(self.request("/v1/sessions", proof)[0], 401)
        proof["signature"] = good
        self.assertEqual(self.request("/v1/sessions", proof)[0], 401)
        token, _ = self.session()
        self.clock += 121
        self.assertEqual(self.request("/v1/hello", token=token)[0], 401)

    def test_pending_ssh_cannot_authenticate_and_tagged_keys_survive_retry_and_revocation(self):
        ssh = self.root / ".ssh"
        ssh.mkdir(mode=0o700)
        unrelated = b'# Keep this comment and unrelated key exactly.\nssh-ed25519 unrelated-existing-key family-admin\n'
        (ssh / "authorized_keys").write_bytes(unrelated)
        bootstrap = self.registry.begin_pairing(self.registration)
        payload = dict(self.registration, pairing_token=bootstrap["pairing_token"])
        self.assertEqual(self.request("/v1/pairings", payload)[0], 200)
        self.assertEqual(self.request("/v1/sessions/challenge", {"board_id": BOARD, "installation_id": INSTALLATION})[0], 401)
        self.registry.activate_pairing(INSTALLATION)
        self.registry.activate_pairing(INSTALLATION)
        keys = (ssh / "authorized_keys").read_bytes()
        self.assertTrue(keys.startswith(unrelated))
        self.assertEqual(keys.count(b"tinkercrew:"), 1)
        self.assertIn(b'restrict,command="', keys)
        self.assertTrue(self.registry.revoke(INSTALLATION)["ssh_key_removed"])
        self.assertEqual((ssh / "authorized_keys").read_bytes(), unrelated)

    def test_revocation_clears_controller_outputs_without_another_client_request(self):
        self.pair()
        token, _ = self.session()
        self.assertEqual(self.request("/v1/control-lease", {"lease_ms": 1500}, token)[0], 200)
        self.assertEqual(self.request("/v1/commands", {"operation": "pixels.set", "arguments": {"color": "#00ff00", "brightness": 0.2, "duration_ms": 1500}}, token)[0], 200)
        self.registry.revoke(INSTALLATION)
        import time
        # Logical lease/session time is frozen: expiry cannot make this pass.
        # Wait for the independently scheduled watchdog, not one arbitrary sleep.
        deadline = time.monotonic() + 1
        while not self.companion.state()["health"]["safe_state"] and time.monotonic() < deadline:
            time.sleep(0.01)
        # Public state only; no authorization-triggering request or manual tick.
        self.assertTrue(self.companion.state()["health"]["safe_state"])

    def test_usb_pairing_is_single_use_and_signed_session_controls_only_until_revoked(self):
        self.assertEqual(self.request("/v1/state")[0], 401)
        payload = self.pair()
        self.assertEqual(self.request("/v1/pairings", payload)[0], 401)
        token, proof = self.session()
        self.assertEqual(self.request("/v1/sessions", proof)[0], 401)
        self.assertEqual(self.request("/v1/hello", token=token)[1]["board"]["board_id"], BOARD)
        self.assertEqual(self.request("/v1/control-lease", {"lease_ms": 1500}, token)[0], 200)
        self.assertEqual(self.request("/v1/commands", {"operation": "pixels.set", "arguments": {"color": "#00ff00", "brightness": 0.2, "duration_ms": 1500}}, token)[0], 200)
        self.registry.revoke(INSTALLATION)
        self.assertEqual(self.request("/v1/state", token=token)[0], 401)
        self.assertEqual(self.request("/v1/commands", {"operation": "outputs.clear", "arguments": {}}, token)[0], 401)
