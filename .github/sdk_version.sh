#!/bin/bash

# Copyright 2024, UNSW
# SPDX-License-Identifier: BSD-2-Clause

set -ex

VERSION=`cat VERSION`
LATEST_TAG=`git describe --tags --abbrev=0`
NUM_COMMITS=`git rev-list --count $LATEST_TAG..HEAD`
HEAD=`git rev-parse --short HEAD`

if [[ $NUM_COMMITS -eq 0 ]];
then
    echo "$VERSION"
else
    VERSION="$VERSION.$NUM_COMMITS+$HEAD"
fi

echo "SDK_VERSION=${VERSION}" >> "${GITHUB_ENV}"
