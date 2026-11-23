"""Signs a Windows release of Iris: writes Iris-Setup-x64.manifest (label, version, the
installer's SHA-256) and its Ed25519 signature, .manifest.sig, beside the installer. Iris
installs a release only when the signature matches a key built into it (update_win.rs).

usage: uv run --with cryptography python sign-release.py <Iris-Setup-x64.exe> <version> <private-key.pem>
The private key never goes in the repository.
"""
import hashlib, sys
from pathlib import Path
from cryptography.hazmat.primitives.serialization import load_pem_private_key

setup, version, key = Path(sys.argv[1]), sys.argv[2].lstrip("v"), Path(sys.argv[3])
sha = hashlib.sha256(setup.read_bytes()).hexdigest()
manifest = f"iris windows release v1\nversion {version}\nsha256 {sha}\n".encode()
private = load_pem_private_key(key.read_bytes(), password=None)
(setup.parent / "Iris-Setup-x64.manifest").write_bytes(manifest)
(setup.parent / "Iris-Setup-x64.manifest.sig").write_bytes(private.sign(manifest))
print(f"signed Iris {version}: sha256 {sha}")
