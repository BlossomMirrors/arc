#!/bin/bash
set -e
./build.sh
RPM=$(ls rpmbuild/RPMS/x86_64/*.rpm)
sudo rpm-ostree usroverlay || true
rpm2cpio $RPM | sudo cpio -fuidmv -D /

sudo update-mime-database /usr/share/mime &>/dev/null || true
sudo update-desktop-database /usr/share/applications &>/dev/null || true
sudo gtk-update-icon-cache /usr/share/icons/hicolor &>/dev/null || true

for MIMEAPPS in /usr/share/applications/mimeapps.list /etc/xdg/mimeapps.list; do
    if [ ! -f "$MIMEAPPS" ]; then
        sudo sh -c "printf '[Default Applications]\n' > '$MIMEAPPS'"
    elif ! sudo grep -q "^\[Default Applications\]" "$MIMEAPPS"; then
        sudo sh -c "printf '\n[Default Applications]\n' >> '$MIMEAPPS'"
    fi
    for ENTRY in \
        "x-scheme-handler/appstream=org.blossomos.Arc.Handler.desktop" \
        "x-scheme-handler/flatpak+https=org.blossomos.Arc.Handler.desktop" \
        "application/vnd.flatpak.ref=org.blossomos.Arc.desktop" \
        "application/vnd.flatpak=org.blossomos.Arc.desktop"
    do
        KEY=$(echo "$ENTRY" | cut -d= -f1)
        sudo sed -i "\|^${KEY}=|d" "$MIMEAPPS"
        sudo sed -i "\|^\[Default Applications\]|a $ENTRY" "$MIMEAPPS"
    done
done
