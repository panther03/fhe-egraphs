#!/bin/bash

./opt_all.sh default > opt_times.csv 2>&1
./verify_all.sh out/opt > ckt_stats.csv 2>&1
./eval_all.sh out/opt > eval_times.csv 2>&1