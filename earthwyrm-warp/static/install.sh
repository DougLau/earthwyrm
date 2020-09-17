#!/bin/sh

SRC=$(dirname "$0")
DST0=/usr/local/bin
DST1=/etc/earthwyrm
DST2=/var/local/earthwyrm
DST3=/etc/systemd/system

cp $SRC/../target/release/earthwyrm-warp $DST0 &&
useradd --system earthwyrm &&
mkdir $DST1 &&
cp $SRC/earthwyrm.muon $DST1 &&
mkdir $DST2 &&
cp $SRC/map.* $DST2 &&
chown --recursive earthwyrm.earthwyrm $DST2 &&
cp $SRC/earthwyrm.service $DST3 &&
echo "Success!"
