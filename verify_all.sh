#!/bin/bash

if [ $# -ne 1 ]
  then
    echo "Please supply bench folder as 1st argument"
    exit 1
fi

# Folder to evaluate
BENCH_FOLDER=$1

CASES=$(for c in $(ls "${BENCH_FOLDER}"/); do echo "BENCH=$(basename $c .eqn)"; done)

parallel make stats verify OPTDIR=$BENCH_FOLDER ::: $CASES