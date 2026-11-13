#!/usr/bin/env python3
"""harness 自签证书生成（mini_ws_tracker wss 臂用，幂等）。"""
from __future__ import annotations

import datetime
import ipaddress
import ssl
from pathlib import Path

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID

CERT_NAME = "mini_ws_tracker"


def _make_key_and_cert() -> tuple[bytes, bytes]:
    key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    subject = issuer = x509.Name(
        [
            x509.NameAttribute(NameOID.COMMON_NAME, CERT_NAME),
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "ezr-harness"),
        ]
    )
    now = datetime.datetime.now(datetime.timezone.utc)
    cert = (
        x509.CertificateBuilder()
        .subject_name(subject)
        .issuer_name(issuer)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(now - datetime.timedelta(days=1))
        .not_valid_after(now + datetime.timedelta(days=3650))
        .add_extension(
            x509.SubjectAlternativeName(
                [
                    x509.DNSName("localhost"),
                    x509.IPAddress(ipaddress.ip_address("127.0.0.1")),
                ]
            ),
            critical=False,
        )
        .sign(key, hashes.SHA256())
    )
    key_pem = key.private_bytes(
        serialization.Encoding.PEM,
        serialization.PrivateFormat.TraditionalOpenSSL,
        serialization.NoEncryption(),
    )
    cert_pem = cert.public_bytes(serialization.Encoding.PEM)
    return cert_pem, key_pem


def ensure_self_signed_cert(cert_dir: Path) -> tuple[Path, Path]:
    cert_dir.mkdir(parents=True, exist_ok=True)
    cert_path = cert_dir / f"{CERT_NAME}.pem"
    key_path = cert_dir / f"{CERT_NAME}-key.pem"
    if not cert_path.exists() or not key_path.exists():
        cert_pem, key_pem = _make_key_and_cert()
        cert_path.write_bytes(cert_pem)
        key_path.write_bytes(key_pem)
    return cert_path, key_path


def server_ssl_context(cert_dir: Path) -> ssl.SSLContext:
    cert, key = ensure_self_signed_cert(cert_dir)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert, key)
    return ctx
