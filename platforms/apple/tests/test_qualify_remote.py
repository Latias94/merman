"""Check the unattended certificate lifecycle without modifying a Keychain."""

import hashlib
from pathlib import Path
import ssl
import subprocess
import tempfile
import unittest
from unittest.mock import call, patch

from platforms.apple.tests import qualify_remote as fixture


class FixtureCertificateTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.cert = Path(directory.name) / "cert.pem"
        certificate_bytes = b"fixture certificate for command boundary tests"
        self.cert.write_text(ssl.DER_cert_to_PEM_cert(certificate_bytes))
        self.keychain = Path("/Library/Keychains/System.keychain")
        self.selector = hashlib.sha1(certificate_bytes).hexdigest()
        self.success = subprocess.CompletedProcess([], 0, stdout="", stderr="")
        self.denied = subprocess.CompletedProcess([], 1, stdout="",
                                                  stderr="SecTrustEvaluate result: kSecTrustResultDeny\n")
        self.patcher = patch.object(fixture, "run")
        self.execute = self.patcher.start()
        self.addCleanup(self.patcher.stop)
        self.execute.side_effect = self.successful_results()

    def successful_results(self):
        return [self.success, self.success, self.success, self.success, self.denied, self.success]

    def expected_commands(self):
        verification = ["security", "verify-cert", "-c", self.cert,
                        "-p", "ssl", "-s", "localhost", "-L"]
        return [
            call(["sudo", "-n", "security", "add-trusted-cert", "-d", "-r", "trustRoot",
                  "-k", self.keychain, self.cert], timeout=30),
            call(verification, timeout=30),
            call(["sudo", "-n", "security", "add-trusted-cert", "-d", "-r", "deny",
                  "-k", self.keychain, self.cert], timeout=30),
            call(["sudo", "-n", "security", "delete-certificate", "-Z", self.selector, self.keychain], timeout=30),
            call(verification, check=False, capture_output=True, text=True, timeout=30),
            call(["security", "find-certificate", "-a", "-Z", self.keychain],
                 capture_output=True, text=True, timeout=30),
        ]

    def test_fixture_is_trusted_then_denied_and_absent(self):
        with fixture.trusted_fixture_certificate(self.cert):
            self.assertEqual(self.execute.call_count, 2)
        self.assertEqual(self.execute.call_args_list, self.expected_commands())

    def test_consumer_failure_still_revokes_trust_and_deletes_certificate(self):
        with self.assertRaisesRegex(RuntimeError, "consumer failed"):
            with fixture.trusted_fixture_certificate(self.cert):
                raise RuntimeError("consumer failed")
        self.assertEqual(self.execute.call_args_list, self.expected_commands())

    def test_failed_or_partial_import_still_revokes_trust_and_deletes_certificate(self):
        self.execute.side_effect = [subprocess.CalledProcessError(1, "security"),
                                    self.success, self.success, self.denied, self.success]
        with self.assertRaises(subprocess.CalledProcessError):
            with fixture.trusted_fixture_certificate(self.cert):
                self.fail("a failed certificate import must not enter the consumer")
        expected = self.expected_commands()
        self.assertEqual(self.execute.call_args_list, expected[:1] + expected[2:])

    def test_failed_trust_verification_does_not_enter_consumer(self):
        results = self.successful_results()
        results[1] = subprocess.CalledProcessError(1, "security")
        self.execute.side_effect = results
        with self.assertRaises(subprocess.CalledProcessError):
            with fixture.trusted_fixture_certificate(self.cert):
                self.fail("an untrusted certificate must not enter the consumer")
        self.assertEqual(self.execute.call_args_list, self.expected_commands())

    def test_failed_or_timed_out_revocation_still_deletes_certificate_and_fails(self):
        for error in (subprocess.CalledProcessError(1, "security"),
                      subprocess.TimeoutExpired("security", 30)):
            with self.subTest(error=error):
                self.execute.reset_mock()
                self.execute.side_effect = [self.success, self.success, error, self.success]
                with self.assertRaises(type(error)):
                    with fixture.trusted_fixture_certificate(self.cert):
                        pass
                self.assertEqual(self.execute.call_args_list, self.expected_commands()[:4])

    def test_failed_certificate_deletion_cannot_report_success(self):
        self.execute.side_effect = [self.success, self.success, self.success,
                                    subprocess.CalledProcessError(1, "security")]
        with self.assertRaises(subprocess.CalledProcessError):
            with fixture.trusted_fixture_certificate(self.cert):
                pass
        self.assertEqual(self.execute.call_args_list, self.expected_commands()[:4])

    def test_cleanup_requires_explicit_deny_not_just_any_verification_failure(self):
        for outcome in (self.success,
                        subprocess.CompletedProcess([], 1, stderr="certificate expired"),
                        subprocess.CompletedProcess([], 2, stderr="kSecTrustResultDeny"),
                        subprocess.TimeoutExpired("security", 30)):
            with self.subTest(outcome=outcome):
                self.execute.reset_mock()
                results = self.successful_results()
                results[4] = outcome
                self.execute.side_effect = results
                with self.assertRaises((RuntimeError, subprocess.TimeoutExpired)):
                    with fixture.trusted_fixture_certificate(self.cert):
                        pass
                self.assertEqual(self.execute.call_args_list, self.expected_commands()[:5])

    def test_remaining_keychain_certificate_prevents_success(self):
        results = self.successful_results()
        results[-1] = subprocess.CompletedProcess([], 0, stdout=f"SHA-1 hash: {self.selector.upper()}\n")
        self.execute.side_effect = results
        with self.assertRaisesRegex(RuntimeError, "remains in the System Keychain"):
            with fixture.trusted_fixture_certificate(self.cert):
                pass
        self.assertEqual(self.execute.call_args_list, self.expected_commands())


if __name__ == "__main__":
    unittest.main()
