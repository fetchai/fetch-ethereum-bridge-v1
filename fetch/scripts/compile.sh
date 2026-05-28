#!/bin/sh

set -xe

wd=$(pwd)

optimize.sh .

cp artifacts/* $wd

case "$*" in
  *--test*)
     cargo test --lib
    ;;
esac

rm -rf registry_cache contract_cache artifacts target
