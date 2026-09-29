FROM docker.io/library/debian:bookworm@sha256:f37a335e82bca302e955fa39f9dfe28f1be618f016f8a2b56318e5a5111afc26

ENV DEBIAN_FRONTEND=noninteractive

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
