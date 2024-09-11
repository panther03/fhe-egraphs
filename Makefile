CKTCONV = ckt-convert/target/release/ckt-convert
EGGTEST = eggtest/target/release/eggtest

BENCH ?= i2c

all: bench
.PHONY: $(CKTCONV) $(EGGTEST)

cktconv: $(CKTCONV)
$(CKTCONV):
	cd ckt-convert && cargo build --release

eggtest: $(EGGTEST)
$(EGGTEST):
	cd eggtest && cargo build --release

bench: $(CKTCONV) $(EGGTEST)
	mkdir -p out/
	$(CKTCONV) convert-eqn lobster_bench/$(BENCH).eqn out/$(BENCH).sexpr
	$(CKTCONV) convert-rules lobster_rules/leave-$(BENCH) out/$(BENCH).rules
	$(EGGTEST) out/$(BENCH).sexpr out/$(BENCH).rules 
