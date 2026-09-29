FROM docker.io/library/debian:bookworm@sha256:f37a335e82bca302e955fa39f9dfe28f1be618f016f8a2b56318e5a5111afc26

ENV DEBIAN_FRONTEND=noninteractive

RUN set -eu; \
    for dpkg_config in /etc/dpkg/dpkg.cfg.d/*; do \
        [ -f "$dpkg_config" ] || continue; \
        sed -i '\|/usr/share/man|d' "$dpkg_config"; \
    done; \
    apt-get update; \
    apt-get install -y --no-install-recommends man-db ninja-build; \
    rm -rf /var/lib/apt/lists/*

COPY verify-installed-package.sh /usr/local/bin/verify-installed-package

RUN chmod 0755 /usr/local/bin/verify-installed-package
