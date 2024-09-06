CKTCONV = ckt-convert/target/release/ckt-convert
EGGTEST = eggtest/target/release/eggtest

BENCH ?= i2c

all: bench
.PHONY: $(CKTCONV) $(EGGTEST)

$(CKTCONV):
	cd ckt-convert && cargo build --release

$(EGGTEST):
	cd eggtest && cargo build --release

bench: $(CKTCONV) $(EGGTEST)
	mkdir -p out/
	$(CKTCONV) lobster_bench/$(BENCH).eqn out/$(BENCH).sexpr convert-eqn
	$(CKTCONV) lobster_rules/leave-$(BENCH) out/$(BENCH).rules convert-rules
	$(EGGTEST) out/$(BENCH).sexpr out/$(BENCH).rules 
