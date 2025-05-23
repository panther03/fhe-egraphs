#!/bin/bash

TEMP_AIG="${1}_temp.aig"
abc -c "read_eqn ${1}; strash; write_aiger ${TEMP_AIG}"
for j in $(seq 1 10); do abc -c "&read ${TEMP_AIG}; &st; &synch2; &if -m -a -K 2; &mfs -W 10; &st; &dch; &if -m -a -K 2; &mfs -W 10; &write ${TEMP_AIG}"; done
abc -c "read_aiger ${TEMP_AIG}; write_eqn ${2}"
rm "${TEMP_AIG}"