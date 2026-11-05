# The release binary is linked on Ubuntu 24.04 and needs GLIBC_2.39, which
# Rocky Linux 9 (2.34) cannot provide. Rocky Linux 10 ships 2.39, so the
# installed binary can actually run the `--version` check.
#
# The declared runtime dependency is deliberately absent: `rpm -i --nodeps`
# skips resolution, and the verifier reads the package's own requirements, so
# an installed copy would only weaken the check.
FROM docker.io/rockylinux/rockylinux:10@sha256:827d37bc128288ccf160ee318bb3cb92d591164cb217e92f8bc61e3982ae1834

RUN set -eu; \
    sed -i -E '/^[[:space:]]*tsflags[[:space:]]*=[[:space:]]*nodocs[[:space:]]*$/d' /etc/dnf/dnf.conf; \
    for rpm_macro in /etc/rpm/macros /etc/rpm/macros.d/*; do \
        [ -f "$rpm_macro" ] || continue; \
        sed -i '/^[[:space:]]*%_excludedocs\b/d' "$rpm_macro"; \
    done; \
    dnf -y install man-db; \
    mandb; \
    man -w man >/dev/null; \
    dnf clean all; \
    rm -rf /var/cache/dnf

COPY verify-installed-package.sh /usr/local/bin/verify-installed-package

RUN chmod 0755 /usr/local/bin/verify-installed-package
