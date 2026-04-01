#!/usr/bin/env bash
set -euo pipefail

if command -v newman >/dev/null 2>&1; then
  exec newman "$@"
fi

if [ -x "tests/postman/node_modules/.bin/newman" ]; then
  exec tests/postman/node_modules/.bin/newman "$@"
fi

if [ -x "/tmp/newman/node_modules/.bin/newman" ]; then
  exec /tmp/newman/node_modules/.bin/newman "$@"
fi

echo "newman is not installed"
echo "Install one of:"
echo "  npm install -g newman"
echo "  npm install --prefix tests/postman newman"
echo "  npm install --prefix /tmp/newman newman"
exit 1
