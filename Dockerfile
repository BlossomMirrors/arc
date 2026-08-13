FROM fedora:44 AS builder

RUN dnf install -y \
        rust \
        cargo \
        pkg-config \
        clang \
        glib2-devel \
        flatpak-devel \
        ostree-devel \
        openssl-devel \
        sqlite-devel \
    && dnf clean all

WORKDIR /build
COPY . .

RUN cargo build --release -p arc-daemon

FROM fedora:44

RUN dnf install -y \
        flatpak \
        flatpak-libs \
        glib2 \
        json-glib \
        ostree-libs \
        polkit-libs \
        ca-certificates \
        curl \
    && dnf clean all

RUN flatpak remote-add --system --if-not-exists \
        flathub https://flathub.org/repo/flathub.flatpakrepo \
    && flatpak remote-add --system --if-not-exists \
        blossomos https://forge.blossomos.org/flatpak.flatpakrepo \
    && flatpak update --appstream --system

COPY --from=builder /build/target/release/arc-daemon /usr/local/bin/arc-daemon

# AppStreamDb only loads once at daemon startup and is never refreshed, so a
# silently empty/broken catalog from the appstream sync above would ship and
# stay broken for the container's whole lifetime. Boot the daemon here and
# fail the build if well-known Flathub apps can't be searched.
RUN set -e; \
    ARC_HTTP_ONLY=1 /usr/local/bin/arc-daemon & \
    daemon_pid=$!; \
    trap 'kill "$daemon_pid" 2>/dev/null || true' EXIT; \
    ready=0; \
    for i in $(seq 1 30); do \
        if curl -sf "http://127.0.0.1:1312/api/v1/search?q=spotify" >/dev/null 2>&1; then ready=1; break; fi; \
        sleep 1; \
    done; \
    [ "$ready" = "1" ] || { echo "arc-daemon did not become ready in time"; exit 1; }; \
    for app in spotify helium; do \
        result=$(curl -sf "http://127.0.0.1:1312/api/v1/search?q=$app"); \
        echo "$result" | grep -q '"id"' || { echo "search for '$app' returned no results: $result"; exit 1; }; \
    done

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

EXPOSE 1312

ENV ARC_HTTP_HOST=0.0.0.0
ENV ARC_HTTP_ONLY=1

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
