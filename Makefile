CKTCONV = ckt-convert/target/release/ckt-convert
EGGTEST = eggtest/target/release/eggtest
HE_EVAL = he-eval/build/he-eval

BENCH ?= i2c
RULESET ?= default

INEQN = bench/$(BENCH).eqn
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
opt: $(OPTDIR) cktconv eggtest $(INEQN) $(INRULES)
	@$(CKTCONV) convert-eqn $(INEQN) out/$(BENCH).sexpr
	@$(CKTCONV) convert-rules $(INRULES) out/$(BENCH).rules
	@$(EGGTEST) out/$(BENCH).sexpr out/$(BENCH).rules > out/$(BENCH)-opt.sexpr
	@$(CKTCONV) convert-sexpr out/$(BENCH)-opt.sexpr $(OPTDIR)/$(BENCH).eqn

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
	@./run_abc.sh $(OPTDIR)/$(BENCH).eqn
