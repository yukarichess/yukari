# If on Windows, add the .exe extension to the executable and use PowerShell instead of `sed`
ifeq ($(OS),Windows_NT)
	EXT := .exe
	NAME := $(shell powershell -Command "(Get-Content yukari/Cargo.toml | Select-String '^name =').Line -replace '.*= ', '' -replace '\"', ''")
	VERSION := $(shell powershell -Command "(Get-Content yukari/Cargo.toml | Select-String '^version =').Line -replace '.*= ', '' -replace '\"', ''")
else
	EXT := 
	NAME := $(shell sed -n 's/^name = "\(.*\)"/\1/p' yukari/Cargo.toml | head -1)
	VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' yukari/Cargo.toml | head -1)

endif

# OpenBench specifies that the binary name should be changeable with the EXE parameter
ifndef EXE
	EXE := $(NAME)-$(VERSION)$(EXT)
else
	EXE := $(EXE)$(EXT)
endif


pgo:
	cargo pgo instrument
	cargo pgo run -- bench
	cargo pgo optimize

# Compile an executable for use with OpenBench
openbench:
	@echo $(NAME)
	@echo Compiling $(EXE) for OpenBench
	cargo rustc --release --manifest-path yukari/Cargo.toml --bin yukari -- -C target-cpu=native --emit link=$(EXE)

# Remove the EXE created
clean:
	@echo Removing $(EXE)
	rm $(EXE)

.PHONY: pgo openbench clean
