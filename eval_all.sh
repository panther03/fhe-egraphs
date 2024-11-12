#!/bin/bash

if [ $# -ne 1 ]
  then
    echo "Please supply bench folder as 1st argument"
    exit 1
fi

# Folder to evaluate
export BENCH_FOLDER=$1

CASES=$(for c in $(ls "${BENCH_FOLDER}"/); do echo "BENCH=$(basename $c .eqn)"; done)

parallel -j 8 ./launch_eval.sh  ::: $CASES 