VENV = .venv/bin/activate
OPENFHE = build/CMakeFiles/OpenFHE

.PHONY: OPENFHE

$(VENV):
	virtualenv openfhe

	source $(VENV); pip install "pybind11[global]"

