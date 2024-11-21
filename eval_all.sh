#!/bin/bash

if [ $# -ne 1 ]
  then
    echo "Usage: $0 <BENCH_FOLDER>"
    exit 1
fi

# Folder to evaluate
export BENCH_FOLDER=$1

CASES=$(for c in $(ls "${BENCH_FOLDER}"/); do echo "BENCH=$(basename $c .eqn)"; done)

parallel -j 1 ./launch_eval.sh  ::: $CASES 