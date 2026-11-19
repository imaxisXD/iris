#!/bin/zsh
# Creates a self-signed "Iris Local Signing" code-signing certificate in your login keychain, so
# scripts/build-app.sh produces builds macOS recognises as the same app (the Accessibility
# permission then survives rebuilds and updates). Only valid on this Mac; not an Apple Developer ID.
# If this Mac already signs Iris with another identity, put its name in .signing-identity instead.
set -euo pipefail
source "${0:A:h}/lib.sh"

NAME=$IRIS_SIGNING_NAME
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -p codesigning | grep -q "\"$NAME\""; then
  echo "\"$NAME\" already exists, nothing to do."
  exit 0
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
PASS=$(openssl rand -hex 16)

openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=$NAME" \
  -addext "basicConstraints=critical,CA:false" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=critical,codeSigning" \
  -keyout "$WORK/key.pem" -out "$WORK/cert.pem" 2>/dev/null

# -legacy: macOS's `security` can't read OpenSSL 3's default PKCS#12 encryption.
openssl pkcs12 -export -legacy -inkey "$WORK/key.pem" -in "$WORK/cert.pem" -name "$NAME" \
  -passout "pass:$PASS" -out "$WORK/iris.p12"

# -T lets codesign use the key without prompting on every build.
security import "$WORK/iris.p12" -k "$KEYCHAIN" -P "$PASS" -T /usr/bin/codesign

security find-identity -p codesigning | grep "\"$NAME\"" && echo "Created \"$NAME\". Now run scripts/build-app.sh."
