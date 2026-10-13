"""Verify short-lived token exchange, secret handling and upload cleanup."""

from contextlib import redirect_stdout, redirect_stderr
import io
import json
import unittest
from unittest import mock
import urllib.error
import urllib.parse

from scripts import crates_io_auth as auth


class TrustedPublishingTests(unittest.TestCase):
    def credential(self):
        return auth.trusted_publishing_token(
            request_url="https://oidc.actions.githubusercontent.com/token?job=42",
            request_token="request-secret",
        )

    def responses(self, *values):
        return [mock.MagicMock(__enter__=mock.Mock(return_value=io.BytesIO(value))) for value in values]

    def test_exchange_uses_audience_masks_secrets_and_revokes_after_upload(self):
        output = io.StringIO()
        responses = self.responses(b'{"value":"jwt-secret"}', b'{"token":"upload-secret"}', b'')
        with mock.patch.object(auth.urllib.request, "urlopen", side_effect=responses) as open_url:
            with redirect_stdout(output), self.credential() as token:
                self.assertEqual(token, "upload-secret")
                self.assertEqual(open_url.call_count, 2)
            requests = [call.args[0] for call in open_url.call_args_list]
        self.assertEqual(urllib.parse.parse_qs(urllib.parse.urlsplit(requests[0].full_url).query),
                         {"job": ["42"], "audience": ["crates.io"]})
        self.assertEqual(requests[0].get_header("Authorization"), "Bearer request-secret")
        self.assertEqual(json.loads(requests[1].data), {"jwt": "jwt-secret"})
        self.assertEqual(requests[1].get_method(), "POST")
        self.assertEqual(requests[2].get_method(), "DELETE")
        self.assertEqual(requests[2].get_header("Authorization"), "Bearer upload-secret")
        self.assertEqual(output.getvalue().splitlines(),
                         ["::add-mask::jwt-secret", "::add-mask::upload-secret"])

    def test_upload_failure_still_revokes_without_hiding_original_error(self):
        responses = self.responses(b'{"value":"jwt-secret"}', b'{"token":"upload-secret"}', b'')
        with mock.patch.object(auth.urllib.request, "urlopen", side_effect=responses) as open_url:
            with redirect_stdout(io.StringIO()), self.assertRaisesRegex(RuntimeError, "upload failed"):
                with self.credential():
                    raise RuntimeError("upload failed")
        self.assertEqual(open_url.call_args.args[0].get_method(), "DELETE")

    def test_http_failure_does_not_expose_response_secrets(self):
        error = urllib.error.HTTPError(auth.TOKENS_URL, 403, "request-secret", {},
                                       io.BytesIO(b'{"error":"jwt-secret"}'))
        responses = self.responses(b'{"value":"jwt-secret"}') + [error]
        with mock.patch.object(auth.urllib.request, "urlopen", side_effect=responses) as open_url:
            with redirect_stdout(io.StringIO()), self.assertRaisesRegex(RuntimeError, "HTTP 403") as raised:
                with self.credential():
                    self.fail("No credential should be returned after a failed exchange")
        self.assertNotIn("secret", str(raised.exception))
        self.assertEqual(open_url.call_count, 2)

    def test_missing_oidc_permission_fails_before_network_access(self):
        with mock.patch.object(auth.urllib.request, "urlopen") as open_url:
            with self.assertRaisesRegex(RuntimeError, "id-token: write"):
                with auth.trusted_publishing_token(request_url=None, request_token=None):
                    self.fail("No credential is available")
        open_url.assert_not_called()

    def test_cleanup_failure_warns_without_retrying_the_upload(self):
        responses = self.responses(b'{"value":"jwt-secret"}', b'{"token":"upload-secret"}')
        responses.append(urllib.error.URLError("private credential details"))
        warning = io.StringIO()
        with mock.patch.object(auth.urllib.request, "urlopen", side_effect=responses):
            with redirect_stdout(io.StringIO()), redirect_stderr(warning):
                with self.credential():
                    pass
        self.assertIn("expires after 30 minutes", warning.getvalue())
        self.assertNotIn("private credential", warning.getvalue())


if __name__ == "__main__":
    unittest.main()
