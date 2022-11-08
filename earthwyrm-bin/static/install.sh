#!/bin/sh

SRC=$(dirname "$0")
BIN_DIR=/usr/local/bin
BASE_DIR=/var/local/earthwyrm
SRV_DIR=/etc/systemd/system

cp $SRC/../target/release/earthwyrm $BIN_DIR &&
useradd --system earthwyrm &&
$BIN_DIR/earthwyrm -b $BASE_DIR init &&
chown --recursive earthwyrm.earthwyrm $BASE_DIR &&
cp $SRC/earthwyrm.service $SRV_DIR &&
echo "Success!"
