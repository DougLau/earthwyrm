#!/bin/sh

SRC=$(dirname "$0")
DST1=/etc/earthwyrm
DST2=/var/local/earthwyrm
DST3=/etc/systemd/system

useradd --system earthwyrm &&
mkdir $DST1 &&
cp $SRC/earthwyrm.muon $DST1 &&
mkdir $DST2 &&
cp $SRC/map.* $DST2 &&
chown --recursive earthwyrm.earthwyrm $DST2 &&
cp $SRC/earthwyrm.service $DST3 &&
echo "Success!"
