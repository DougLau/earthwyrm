#!/bin/sh

SRC=$(dirname "$0")
BIN_DIR=/usr/local/bin
ETC_DIR=/etc/earthwyrm
VAR_DIR=/var/local/earthwyrm
OSM_DIR=$VAR_DIR/osm
STATIC_DIR=$VAR_DIR/static
LOAM_DIR=$VAR_DIR/loam
SRV_DIR=/etc/systemd/system

cp $SRC/../target/release/earthwyrm $BIN_DIR &&
useradd --system earthwyrm &&
mkdir $ETC_DIR &&
cp $SRC/earthwyrm.muon $ETC_DIR &&
mkdir $VAR_DIR &&
mkdir $OSM_DIR &&
mkdir $STATIC_DIR &&
mkdir $LOAM_DIR &&
cp $SRC/index.html $STATIC_DIR &&
cp $SRC/map.* $STATIC_DIR &&
chown --recursive earthwyrm.earthwyrm $VAR_DIR &&
cp $SRC/earthwyrm.service $SRV_DIR &&
echo "Success!"
