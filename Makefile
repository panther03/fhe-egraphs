CKTCONV = ckt-convert/target/release/ckt-convert
EGGTEST = eggtest/target/release/eggtest
CKTVERIFY = ckt-verify/build/ckt-verify

BENCH ?= i2c

all: bench
.PHONY: $(CKTCONV) $(EGGTEST)

cktconv: $(CKTCONV)
$(CKTCONV):
	cd ckt-convert && cargo build --release

eggtest: $(EGGTEST)
$(EGGTEST):
	cd eggtest && cargo build --release

cktverify: $(CKTVERIFY)
$(CKTVERIFY):
	cd ckt-verify && cmake -B build && cmake --build build/

# $(CKTCONV) $(EGGTEST)
bench: 
	@mkdir -p out/
	@$(CKTCONV) convert-eqn lobster_bench/$(BENCH).eqn out/$(BENCH).sexpr
# if lobster_result/$(BENCH).eqn_opted_result exists, convert it
	@if [ -f lobster_result/$(BENCH).eqn_opted_result ]; then \
		$(CKTCONV) convert-eqn lobster_result/$(BENCH).eqn_opted_result out/$(BENCH)-lob.sexpr; \
	fi
	@$(CKTCONV) convert-rules lobster_rules/leave-$(BENCH) out/$(BENCH).rules
	@$(EGGTEST) out/$(BENCH).sexpr out/$(BENCH).rules > out/$(BENCH)-opt.sexpr
	@echo -n "$(BENCH),"
	@$(CKTCONV) sexpr-stats out/$(BENCH).sexpr
	@$(CKTCONV) sexpr-stats out/$(BENCH)-opt.sexpr
# if we converted out/($(BENCH)-lob.sexpr), print stats, otherwise print -1,-1,
	@if [ -f lobster_result/$(BENCH).eqn_opted_result ]; then \
		$(CKTCONV) sexpr-stats out/$(BENCH)-lob.sexpr; \
	else \
		echo -n "-1,-1,"; \
	fi

verify: bench
	$(CKTCONV) convert-sexpr out/$(BENCH)-opt.sexpr out/$(BENCH)-opt.eqn
	@./run_abc.sh $(BENCH)
	