#!/bin/sh

SRC=$(dirname "$0")
BIN_DIR=/usr/local/bin
ETC_DIR=/etc/earthwyrm
BASE_DIR=/var/local/earthwyrm
OSM_DIR=$BASE_DIR/osm
STATIC_DIR=$BASE_DIR/static
LOAM_DIR=$BASE_DIR/loam
SRV_DIR=/etc/systemd/system

cp $SRC/../target/release/earthwyrm $BIN_DIR &&
useradd --system earthwyrm &&
mkdir $ETC_DIR &&
cp $SRC/earthwyrm.muon $ETC_DIR &&
mkdir $BASE_DIR &&
mkdir $OSM_DIR &&
mkdir $STATIC_DIR &&
mkdir $LOAM_DIR &&
cp $SRC/index.html $STATIC_DIR &&
cp $SRC/map.* $STATIC_DIR &&
chown --recursive earthwyrm.earthwyrm $BASE_DIR &&
cp $SRC/earthwyrm.service $SRV_DIR &&
echo "Success!"
