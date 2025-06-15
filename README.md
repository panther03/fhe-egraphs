# [Preprint](doc/fhe_egraphs25.pdf)

# For modified XAG optimization algorithms, see [mockturtle](https://github.com/panther03/mockturtle/tree/tracing_rewrite)

# Repo structure

* `ckt-convert/`: utilities for dealing with parsing/writing logic networks
* `eqsat-opt`: `egg`-based utility for replaying trace file and extracting to optimize MC/MD cost
* `he-eval`: homomorphic graph evaluator code for experiments
* `rules`: base rewrite rules used for experiments with eqsat
* `scripts`: miscellaneous scripts

Note: benchmarks/rules are sourced from https://github.com/ropas/PLDI2020_242_artifact_publication