#!/bin/sh

SRC=$(dirname "$0")
DST1=/etc/earthwyrm
DST2=/var/lib/earthwyrm
DST3=/etc/systemd/system

mkdir $DST1 &&
cp $SRC/earthwyrm.toml $DST1 &&
cp $SRC/earthwyrm.rules $DST1 &&
mkdir $DST2 &&
cp $SRC/map.* $DST2 &&
cp $SRC/earthwyrm.service $DST3 &&
useradd --system earthwyrm &&
echo "Success!"
