#!/bin/bash

# Check if two arguments are provided
for eqn in $1/*.eqn; do
    echo "$eqn"
    abc -c "read_lib /home/julien/asap7_clean.lib; read_eqn $eqn; strash; dch -f; map; topo; upsize; dnsize; stime -d" | grep WireLoad
    cat stats.txt >> all_stats.txt
done