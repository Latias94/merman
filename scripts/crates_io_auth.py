#!/usr/bin/env python3
"""Acquire and revoke a fresh crates.io OIDC token around one upload."""

from contextlib import contextmanager
import json
import sys
import urllib.error
import urllib.parse
import urllib.request


TOKENS_URL = "https://crates.io/api/v1/trusted_publishing/tokens"
USER_AGENT = "merman-release (+https://github.com/Latias94/merman)"


def _request(request: urllib.request.Request, operation: str) -> bytes:
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        raise RuntimeError(f"{operation} failed with HTTP {error.code}") from None
    except OSError:
        raise RuntimeError(f"{operation} failed; check the service connection") from None


def _secret(response: bytes, key: str) -> str:
    try:
        value = json.loads(response)[key]
    except (ValueError, KeyError, TypeError):
        raise RuntimeError("Authentication service returned an invalid token response") from None
    if not isinstance(value, str) or not value or any(char.isspace() for char in value):
        raise RuntimeError("Authentication service returned an invalid token")
    print(f"::add-mask::{value}", flush=True)
    return value


@contextmanager
def trusted_publishing_token(*, request_url: str | None, request_token: str | None):
    if not request_url or not request_token:
        raise RuntimeError("Trusted Publishing requires GitHub Actions id-token: write")
    url = urllib.parse.urlsplit(request_url)
    if url.scheme != "https":
        raise RuntimeError("GitHub OIDC request URL must use HTTPS")
    query = urllib.parse.parse_qsl(url.query, keep_blank_values=True)
    query = [(key, value) for key, value in query if key != "audience"]
    query.append(("audience", "crates.io"))
    oidc_url = urllib.parse.urlunsplit(url._replace(query=urllib.parse.urlencode(query)))
    jwt = _secret(_request(urllib.request.Request(
        oidc_url, headers={"Authorization": f"Bearer {request_token}"},
    ), "GitHub OIDC token request"), "value")
    token = _secret(_request(urllib.request.Request(
        TOKENS_URL, data=json.dumps({"jwt": jwt}).encode(),
        headers={"Content-Type": "application/json", "User-Agent": USER_AGENT},
        method="POST",
    ), "crates.io token exchange"), "token")
    try:
        yield token
    finally:
        try:
            _request(urllib.request.Request(
                TOKENS_URL, headers={"Authorization": f"Bearer {token}", "User-Agent": USER_AGENT},
                method="DELETE",
            ), "crates.io token revocation")
        except RuntimeError as error:
            print(f"::warning::{error}; the temporary token expires after 30 minutes", file=sys.stderr)
