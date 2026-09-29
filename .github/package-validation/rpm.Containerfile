FROM docker.io/library/rockylinux:9@sha256:d7be1c094cc5845ee815d4632fe377514ee6ebcf8efaed6892889657e5ddaaa6

RUN set -eu; \
    sed -i -E '/^[[:space:]]*tsflags[[:space:]]*=[[:space:]]*nodocs[[:space:]]*$/d' /etc/dnf/dnf.conf; \
    for rpm_macro in /etc/rpm/macros /etc/rpm/macros.d/*; do \
        [ -f "$rpm_macro" ] || continue; \
        sed -i '/^[[:space:]]*%_excludedocs\b/d' "$rpm_macro"; \
    done; \
    dnf -y install man-db; \
    dnf clean all; \
    rm -rf /var/cache/dnf

COPY verify-installed-package.sh /usr/local/bin/verify-installed-package

RUN chmod 0755 /usr/local/bin/verify-installed-package
