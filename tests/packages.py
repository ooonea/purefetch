"""Compare detection with each distribution's native installed-package query."""
import re
from pathlib import Path
import subprocess
import sys


binary = str(Path(sys.argv[1]).resolve())
for image, manager, query in [
    ("fedora:latest", "rpm", "rpm -qa --queryformat '%{NAME}\\n' | grep -vx gpg-pubkey | wc -l"),
    ("debian:stable-slim", "dpkg", "dpkg-query -W -f='${db:Status-Status}\\n' | grep -cx installed"),
    ("archlinux:base", "pacman", "pacman -Qq | wc -l"),
    ("alpine:latest", "apk", "apk info | wc -l"),
    ("ghcr.io/void-linux/void-glibc:latest", "xbps", "xbps-query -l | grep -c '^ii '"),
    ("gentoo/stage3:latest", "emerge", "python3 -c 'import portage; print(len(portage.db[\"/\"][\"vartree\"].dbapi.cpv_all()))'"),
]:
    result = subprocess.run(
        ["docker", "run", "--rm", "--network=none", "-v", f"{binary}:/purefetch:ro",
         "--entrypoint", "/bin/sh", image, "-ec",
         f"{query}; /purefetch --no-config --no-logo --modules packages"],
        capture_output=True, text=True, timeout=300, check=True,
    )
    expected, output = result.stdout.strip().split("\n", 1)
    count = int(expected)
    actual = re.search(rf"(\d+) \({manager}\)", output)
    assert count > 0 and actual and int(actual[1]) == count, result.stdout
    print(f"{image}: {count} ({manager})", flush=True)
