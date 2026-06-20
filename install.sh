#!/bin/bash
./build.sh
RPM=$(ls rpmbuild/RPMS/x86_64/*.rpm)
sudo rpm-ostree usroverlay || true
rpm2cpio $RPM | sudo cpio -fuidmv -D /
