EVALFILE ?= ../../../fti20-ladybower.bin

# If on Windows, add the .exe extension to the executable and use PowerShell instead of `sed`
ifeq ($(OS),Windows_NT)
	EXT := .exe
	NAME := $(shell powershell -Command "(Get-Content yukari/Cargo.toml | Select-String '^name =').Line -replace '.*= ', '' -replace '\"', ''")
	VERSION := $(shell powershell -Command "(Get-Content yukari/Cargo.toml | Select-String '^version =').Line -replace '.*= ', '' -replace '\"', ''")
	TRIPLE := $(shell powershell -Command "(rustc -vV | Select-String '^host: ').Line -replace '^host: ', ''")
else
	EXT := 
	NAME := $(shell sed -n 's/^name = "\(.*\)"/\1/p' yukari/Cargo.toml | head -1)
	VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' yukari/Cargo.toml | head -1)
	TRIPLE := $(shell rustc -vV | sed -n "s/host: //p")
endif

# OpenBench specifies that the binary name should be changeable with the EXE parameter
ifndef EXE
	EXE := $(NAME)-$(VERSION)$(EXT)
else
	EXE := $(EXE)$(EXT)
endif


# Compile an executable for use with OpenBench
openbench:
	@echo $(NAME)
	@echo Compiling $(EXE) for OpenBench
	@echo "triple: $(TRIPLE)"
	rustup component add llvm-tools
	cargo install cargo-pgo
	mkdir -p .cargo
	echo "[target.$(TRIPLE)]" > .cargo/config.toml
	echo "rustflags = \"-C target-cpu=native\"" >> .cargo/config.toml
	echo "[env]" >> .cargo/config.toml
	echo "EVALFILE = \"$(EVALFILE)\"" >> .cargo/config.toml
	cargo pgo instrument
	cargo pgo run -- bench
	cargo pgo optimize
	mv "target/$(TRIPLE)/release/yukari" "$(EXE)"

# Remove the EXE created
clean:
	@echo Removing $(EXE)
	rm $(EXE)

.PHONY: openbench clean

