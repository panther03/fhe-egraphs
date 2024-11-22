CKTCONV = ckt-convert/target/release/ckt-convert
EGGTEST = eggtest/target/release/eggtest
HE_EVAL = he-eval/build/he-eval

BENCH ?= i2c
RULESET ?= default
BENCHSET ?= lobster

INEQN = bench/$(BENCHSET)/$(BENCH).eqn
INRULES = rules/$(RULESET)/leave-$(BENCH)
OPTDIR ?= out/opt/

all: verify stats eval
#.PHONY: $(CKTCONV) $(EGGTEST) 

cktconv: $(CKTCONV)
$(CKTCONV):
	cd ckt-convert && cargo build --release

eggtest: $(EGGTEST)
$(EGGTEST):
	cd eggtest && cargo build --release

he-eval: $(HE_EVAL)
$(HE_EVAL):
	cd he-eval && cmake -B build && cmake --build build/

$(OPTDIR):
	@mkdir -p $(OPTDIR)

# optimize a single file
opt: $(OPTDIR) cktconv eggtest $(INEQN)
	@$(CKTCONV) eqn2seqn $(INEQN) out/$(BENCH).seqn
	@if [ -f "$(INRULES)" ]; then \
		$(CKTCONV) lobster2egg-rules $(INRULES) out/$(BENCH).rules; \
	else \
		$(CKTCONV) lobster2egg-rules rules/$(RULESET)/all_cases out/$(BENCH).rules; \
	fi
	@if [ "$(BENCHSET)" = "iscas" ]; then \
		TIMEOUT=60; \
	else \
		TIMEOUT=$$( $(CKTCONV) stats $(INEQN) | awk -F',' '{print int($$1 * $$1 * $$2 / 10000 * 60)}' ); \
	fi; \
	$(EGGTEST) md $$TIMEOUT out/$(BENCH).seqn out/$(BENCH).rules $(OPTDIR)/$(BENCH).eqn

# homomorphic evaluation
eval: $(OPTDIR)/$(BENCH).eqn he-eval
	@$(HE_EVAL) -q $(OPTDIR)/$(BENCH).eqn

# show stats of eqn
stats: $(OPTDIR)/$(BENCH).eqn
	@echo -n "$(BENCH),"
	@$(CKTCONV) stats $(OPTDIR)/$(BENCH).eqn
	@echo -n ","

# verify against ABC
verify: $(OPTDIR)/$(BENCH).eqn
	@./run_abc.sh $(INEQN) $(OPTDIR)/$(BENCH).eqn
