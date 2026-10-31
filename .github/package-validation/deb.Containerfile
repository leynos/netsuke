# The release binary is linked on Ubuntu 24.04 and needs GLIBC_2.39, which
# Debian bookworm (2.36) cannot provide. Trixie ships 2.41, so the installed
# binary can actually run the `--version` check.
FROM docker.io/library/debian:trixie@sha256:2dc419c6d7fe3dad0a2f9fd469c34cc90b0f9ba642d8fde837fe3d7d34a0e7f1

ENV DEBIAN_FRONTEND=noninteractive

# `ninja-build` is installed, not because the check needs it present, but
# because `dpkg -i` refuses to install a package whose declared dependencies
# are missing. The verifier reads the package's Depends field, so an installed
# copy cannot make an undeclared dependency pass.
RUN set -eu; \
    for dpkg_config in /etc/dpkg/dpkg.cfg /etc/dpkg/dpkg.cfg.d/*; do \
        [ -f "$dpkg_config" ] || continue; \
        sed -i -E '\|^[[:space:]]*path-exclude[[:space:]]*=[[:space:]]*/usr/share/man(/[^[:space:]]*)?([[:space:]]*(#.*)?)?$|d' "$dpkg_config"; \
        if grep -Eq '^[[:space:]]*path-exclude[[:space:]]*=[[:space:]]*/usr/share/man(/[^[:space:]]*)?([[:space:]]*(#.*)?)?$' "$dpkg_config"; then \
            printf 'FAIL: dpkg still excludes /usr/share/man in %s\n' "$dpkg_config" >&2; \
            exit 1; \
        fi; \
    done; \
    apt-get update; \
    apt-get install -y --no-install-recommends man-db ninja-build; \
    mandb; \
    man -w man >/dev/null; \
    rm -rf /var/lib/apt/lists/*

COPY verify-installed-package.sh /usr/local/bin/verify-installed-package

RUN chmod 0755 /usr/local/bin/verify-installed-package
