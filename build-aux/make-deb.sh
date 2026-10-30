#!/bin/sh
# Build the Debian/Ubuntu binary package: ../annota_<version>_<arch>.deb
# Usage: build-aux/make-deb.sh   (needs the Build-Depends from debian/control)
set -eu
cd "$(dirname "$0")/.."
build-aux/vendor.sh
dpkg-buildpackage -us -uc -b
lintian -I --pedantic --fail-on error,warning ../annota_*.deb
echo "Done:"; ls -1 ../annota_*.deb
