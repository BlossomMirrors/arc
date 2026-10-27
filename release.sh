#!/bin/bash
set -e

SRC_DIR=$(pwd)
TOOLBOX=arc-build

if [ -f /run/.toolboxenv ]; then
    echo "=== Installing build dependencies in the toolbox ==="
    sudo dnf install -y rpm-build rust cargo clang pkg-config gettext-devel \
        glib2-devel flatpak-devel ostree-devel openssl-devel sqlite-devel \
        qt6-qtbase-devel qt6-qtdeclarative-devel kf6-kirigami-devel kf6-ki18n-devel \
        kf6-kdbusaddons-devel kf6-kwindowsystem-devel
elif command -v toolbox >/dev/null 2>&1; then
    # build in a toolbox so compilers and devel packages never touch the system image
    FEDORA=$(. /etc/os-release && echo "$VERSION_ID")
    if ! toolbox list --containers | grep -qw "$TOOLBOX"; then
        echo "=== Creating the $TOOLBOX toolbox on Fedora $FEDORA ==="
        toolbox create -y -r "$FEDORA" "$TOOLBOX"
    fi
    # the shared home puts linuxbrew and distrobox tools in the path so keep only the toolbox system tools
    exec toolbox run -c "$TOOLBOX" env PATH=/usr/local/bin:/usr/bin bash -c 'cd "$1" && shift && ./release.sh "$@"' _ "$SRC_DIR" "$@"
fi

PACKAGE_NAME=blossom-arc
VERSION=0.4.0
RELEASE=1
BUILDROOT=$(pwd)/rpmbuild
SPECS_DIR=$BUILDROOT/SPECS
SOURCES_DIR=$BUILDROOT/SOURCES

echo "Building Arc..."
cargo build --release --workspace

echo "Generating shell completions..."
mkdir -p completions
target/release/arc completions bash > completions/arc.bash
target/release/arc completions zsh  > completions/_arc
target/release/arc completions fish > completions/arc.fish

echo "Compiling frontend translations..."
rm -rf locale
for PO in crates/arc-frontend/locales/*.po; do
    LANG_CODE=$(basename "$PO" .po)
    mkdir -p locale/$LANG_CODE/LC_MESSAGES
    msgfmt -o locale/$LANG_CODE/LC_MESSAGES/arc-frontend.mo "$PO"
done

rm -rf $BUILDROOT
mkdir -p $SPECS_DIR $SOURCES_DIR

# Generate XDG autostart entry for the daemon
cat > arc-daemon.desktop <<'AUTOSTART'
[Desktop Entry]
Type=Application
Name=Arc Daemon
Comment=Background service for Arc software center
Exec=/usr/bin/arc-daemon
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
AUTOSTART

# the frontend waits hidden after login so opening it only has to show the window
cat > arc-frontend-standby.desktop <<'AUTOSTART'
[Desktop Entry]
Type=Application
Name=Arc Store Standby
Comment=Prepares the Arc window in the background
Exec=/usr/bin/arc-frontend --standby
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
AUTOSTART

# the launcher activates the waiting window over dbus instead of starting a process
cat > org.blossomos.Arc.service <<'SERVICE'
[D-BUS Service]
Name=org.blossomos.Arc
Exec=/usr/bin/arc-frontend
SERVICE
cat > org.blossomos.Arc.Handler.service <<'SERVICE'
[D-BUS Service]
Name=org.blossomos.Arc.Handler
Exec=/usr/bin/arc-frontend
SERVICE

# Bundle pre-built binaries + data files into source tarball
tar -czf $SOURCES_DIR/$PACKAGE_NAME-$VERSION.tar.gz \
    --transform "s|^|$PACKAGE_NAME-$VERSION/|" \
    target/release/arc-frontend \
    target/release/arc-daemon \
    target/release/arc \
    org.blossomos.Arc.desktop \
    org.blossomos.Arc.Handler.desktop \
    org.blossomos.Arc.metainfo.xml \
    org.blossomos.Arc.xml \
    arc-daemon.desktop \
    arc-frontend-standby.desktop \
    org.blossomos.Arc.service \
    org.blossomos.Arc.Handler.service \
    arc.svg \
    LICENSE \
    completions/arc.bash \
    completions/_arc \
    completions/arc.fish \
    locale

SPECFILE=$SPECS_DIR/$PACKAGE_NAME.spec

cat > $SPECFILE <<EOF
%global debug_package %{nil}

Name:           $PACKAGE_NAME
Version:        $VERSION
Release:        $RELEASE%{?dist}
Summary:        Software center for Flatpak and local package file installation
License:        MIT
URL:            https://git.blossomos.org/Blossom/arc

Source0:        $PACKAGE_NAME-$VERSION.tar.gz

Requires:       flatpak-libs
Requires:       distrobox
Requires:       flatpak

BuildRequires:  tar

%description
Arc is a software center for BlossomOS that lets you browse and install
Flatpak applications from Flathub and install local package files
(.deb, .rpm, .pkg.tar.zst) into isolated Distrobox containers with
automatic desktop integration.

%prep
%setup -q

%build
# pre-built by release.sh

%install
install -Dm 755 target/release/arc-frontend %{buildroot}/usr/bin/arc-frontend
install -Dm 755 target/release/arc-daemon   %{buildroot}/usr/bin/arc-daemon
install -Dm 755 target/release/arc      %{buildroot}/usr/bin/arc
install -Dm 644 org.blossomos.Arc.desktop         %{buildroot}/usr/share/applications/org.blossomos.Arc.desktop
install -Dm 644 org.blossomos.Arc.Handler.desktop %{buildroot}/usr/share/applications/org.blossomos.Arc.Handler.desktop
install -Dm 644 org.blossomos.Arc.metainfo.xml %{buildroot}/usr/share/metainfo/org.blossomos.Arc.metainfo.xml
install -Dm 644 org.blossomos.Arc.xml         %{buildroot}/usr/share/mime/packages/org.blossomos.Arc.xml
install -Dm 644 arc-daemon.desktop            %{buildroot}/etc/xdg/autostart/arc-daemon.desktop
install -Dm 644 arc-frontend-standby.desktop  %{buildroot}/etc/xdg/autostart/arc-frontend-standby.desktop
install -Dm 644 org.blossomos.Arc.service     %{buildroot}/usr/share/dbus-1/services/org.blossomos.Arc.service
install -Dm 644 org.blossomos.Arc.Handler.service %{buildroot}/usr/share/dbus-1/services/org.blossomos.Arc.Handler.service
install -Dm 644 arc.svg                       %{buildroot}/usr/share/icons/hicolor/scalable/apps/org.blossomos.Arc.svg
install -Dm 644 LICENSE                       %{buildroot}/usr/share/licenses/$PACKAGE_NAME/LICENSE
install -Dm 644 completions/arc.bash          %{buildroot}/usr/share/bash-completion/completions/arc
install -Dm 644 completions/_arc              %{buildroot}/usr/share/zsh/site-functions/_arc
install -Dm 644 completions/arc.fish          %{buildroot}/usr/share/fish/vendor_completions.d/arc.fish
mkdir -p %{buildroot}/usr/share/locale
cp -r locale/* %{buildroot}/usr/share/locale/

%find_lang arc-frontend

%post
update-mime-database /usr/share/mime &>/dev/null || :
update-desktop-database /usr/share/applications &>/dev/null || :
gtk-update-icon-cache /usr/share/icons/hicolor &>/dev/null || :
for MIMEAPPS in /usr/share/applications/mimeapps.list /etc/xdg/mimeapps.list; do
    if [ ! -f "\$MIMEAPPS" ]; then
        printf '[Default Applications]\n' > "\$MIMEAPPS"
    elif ! grep -q "^\[Default Applications\]" "\$MIMEAPPS"; then
        printf '\n[Default Applications]\n' >> "\$MIMEAPPS"
    fi
    for ENTRY in \
        "x-scheme-handler/appstream=org.blossomos.Arc.Handler.desktop" \
        "x-scheme-handler/flatpak+https=org.blossomos.Arc.Handler.desktop" \
        "application/vnd.flatpak.ref=org.blossomos.Arc.desktop" \
        "application/vnd.flatpak=org.blossomos.Arc.desktop"
    do
        KEY=\$(echo "\$ENTRY" | cut -d= -f1)
        sed -i "\|^\${KEY}=|d" "\$MIMEAPPS"
        sed -i "\|^\[Default Applications\]|a \$ENTRY" "\$MIMEAPPS"
    done
done

%postun
update-mime-database /usr/share/mime &>/dev/null || :
update-desktop-database /usr/share/applications &>/dev/null || :
gtk-update-icon-cache /usr/share/icons/hicolor &>/dev/null || :
for MIMEAPPS in /usr/share/applications/mimeapps.list /etc/xdg/mimeapps.list; do
    sed -i \
        '\|x-scheme-handler/appstream=org\.blossomos\.Arc\.Handler\.desktop|d
         \|x-scheme-handler/flatpak+https=org\.blossomos\.Arc\.Handler\.desktop|d
         \|application/vnd\.flatpak\.ref=org\.blossomos\.Arc\.desktop|d
         \|application/vnd\.flatpak=org\.blossomos\.Arc\.desktop|d' \
        "\$MIMEAPPS" 2>/dev/null || :
done

%files -f arc-frontend.lang
/usr/bin/arc-frontend
/usr/bin/arc-daemon
/usr/bin/arc
/usr/share/applications/org.blossomos.Arc.desktop
/usr/share/applications/org.blossomos.Arc.Handler.desktop
/usr/share/metainfo/org.blossomos.Arc.metainfo.xml
/usr/share/mime/packages/org.blossomos.Arc.xml
/etc/xdg/autostart/arc-daemon.desktop
/etc/xdg/autostart/arc-frontend-standby.desktop
/usr/share/dbus-1/services/org.blossomos.Arc.service
/usr/share/dbus-1/services/org.blossomos.Arc.Handler.service
/usr/share/icons/hicolor/scalable/apps/org.blossomos.Arc.svg
%license /usr/share/licenses/$PACKAGE_NAME/LICENSE
/usr/share/bash-completion/completions/arc
/usr/share/zsh/site-functions/_arc
/usr/share/fish/vendor_completions.d/arc.fish

%changelog
* $(LANG=C date +"%a %b %d %Y") Blossom Labs <hello@blossomos.org> - $VERSION-$RELEASE
- Initial release
EOF

rpmbuild -bb $SPECFILE \
    --define "_topdir $BUILDROOT" \
    --define "_sourcedir $SOURCES_DIR"

echo ""
echo "Build complete! RPM package:"
find $BUILDROOT/RPMS -name "*.rpm"
