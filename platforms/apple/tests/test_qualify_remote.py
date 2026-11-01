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
        self.patcher = patch.object(fixture, "run", return_value=subprocess.CompletedProcess([], 0))
        self.execute = self.patcher.start()
        self.addCleanup(self.patcher.stop)

    def expected_commands(self):
        return [
            call(["sudo", "-n", "security", "add-trusted-cert", "-d", "-r", "trustRoot",
                  "-k", self.keychain, self.cert], timeout=30),
            call(["sudo", "-n", "security", "remove-trusted-cert", "-d", self.cert],
                 check=False, timeout=30),
            call(["sudo", "-n", "security", "delete-certificate", "-Z", self.selector, self.keychain],
                 check=False, timeout=30),
        ]

    def test_same_administrator_domain_and_exact_certificate_are_cleaned(self):
        with fixture.trusted_fixture_certificate(self.cert):
            self.assertEqual(self.execute.call_count, 1)
        self.assertEqual(self.execute.call_args_list, self.expected_commands())

    def test_consumer_failure_still_cleans_both_trust_and_certificate(self):
        with self.assertRaisesRegex(RuntimeError, "consumer failed"):
            with fixture.trusted_fixture_certificate(self.cert):
                raise RuntimeError("consumer failed")
        self.assertEqual(self.execute.call_args_list, self.expected_commands())

    def test_failed_import_still_attempts_cleanup(self):
        self.execute.side_effect = [subprocess.CalledProcessError(1, "security"),
                                    subprocess.CompletedProcess([], 0), subprocess.CompletedProcess([], 0)]
        with self.assertRaises(subprocess.CalledProcessError):
            with fixture.trusted_fixture_certificate(self.cert):
                self.fail("a failed certificate import must not enter the consumer")
        self.assertEqual(self.execute.call_args_list, self.expected_commands())

    def test_cleanup_failures_cannot_report_success(self):
        for remove_code, delete_code in ((1, 0), (0, 1), (1, 1)):
            with self.subTest(remove=remove_code, delete=delete_code):
                self.execute.reset_mock()
                self.execute.side_effect = [subprocess.CompletedProcess([], 0),
                                            subprocess.CompletedProcess([], remove_code),
                                            subprocess.CompletedProcess([], delete_code)]
                with self.assertRaisesRegex(RuntimeError, "could not remove"):
                    with fixture.trusted_fixture_certificate(self.cert):
                        pass
                self.assertEqual(self.execute.call_args_list, self.expected_commands())

    def test_timed_out_trust_removal_still_attempts_certificate_deletion(self):
        self.execute.side_effect = [subprocess.CompletedProcess([], 0),
                                    subprocess.TimeoutExpired("security", 30), subprocess.CompletedProcess([], 0)]
        with self.assertRaises(subprocess.TimeoutExpired):
            with fixture.trusted_fixture_certificate(self.cert):
                pass
        self.assertEqual(self.execute.call_args_list, self.expected_commands())


if __name__ == "__main__":
    unittest.main()
