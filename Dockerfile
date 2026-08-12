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
    && dnf clean all

RUN flatpak remote-add --system --if-not-exists \
        flathub https://flathub.org/repo/flathub.flatpakrepo \
    && flatpak remote-add --system --if-not-exists \
        blossomos https://forge.blossomos.org/flatpak.flatpakrepo \
    && flatpak update --appstream --system || true

COPY --from=builder /build/target/release/arc-daemon /usr/local/bin/arc-daemon

EXPOSE 1312

ENV ARC_HTTP_HOST=0.0.0.0
ENV ARC_HTTP_ONLY=1

CMD ["/usr/local/bin/arc-daemon"]
