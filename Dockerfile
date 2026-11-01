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
        blossomos https://forge.arcstore.net/flatpak.flatpakrepo \
    && flatpak update --appstream --system

COPY --from=builder /build/target/release/arc-daemon /usr/local/bin/arc-daemon

# AppStreamDb only loads once at daemon startup and is never refreshed, so a
# silently empty/broken catalog from the appstream sync above would ship and
# stay broken for the container's whole lifetime. Boot the daemon here and
# fail the build if well-known Flathub apps can't be searched.
# The daemon serves HTTP before the catalogue sync finishes (each remote is
# ingested in the background and committed in one transaction), so an early
# empty result is expected: keep polling until every app shows up.
RUN set -e; \
    ARC_HTTP_ONLY=1 /usr/local/bin/arc-daemon & \
    daemon_pid=$!; \
    trap 'kill "$daemon_pid" 2>/dev/null || true' EXIT; \
    for app in spotify helium; do \
        found=0; result=""; \
        for i in $(seq 1 180); do \
            kill -0 "$daemon_pid" 2>/dev/null || { echo "arc-daemon exited unexpectedly"; exit 1; }; \
            result=$(curl -sf "http://127.0.0.1:1312/api/v1/search?q=$app" 2>/dev/null || true); \
            if echo "$result" | grep -q '"id"'; then found=1; break; fi; \
            sleep 1; \
        done; \
        [ "$found" = "1" ] || { echo "search for '$app' returned no results after 180s: $result"; exit 1; }; \
    done

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

EXPOSE 1312

ENV ARC_HTTP_HOST=0.0.0.0
ENV ARC_HTTP_ONLY=1

CMD ["/usr/local/bin/entrypoint.sh"]
