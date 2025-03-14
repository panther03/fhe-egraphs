CKTCONV = ckt-convert/target/release/ckt-convert
EQSATOPT = eqsat-opt/target/release/eqsat-opt
HE_EVAL = he-eval/build/he-eval

all: cktconv eqsatopt he-eval
.PHONY: $(CKTCONV) $(EQSATOPT) $(HE_EVAL)

cktconv: $(CKTCONV)
$(CKTCONV):
	cd ckt-convert && cargo build --release

eqsatopt: $(EQSATOPT)
$(EQSATOPT):
	cd eqsat-opt && cargo build --release

he-eval: $(HE_EVAL)
$(HE_EVAL):
	cd he-eval && cmake -B build && cmake --build build/