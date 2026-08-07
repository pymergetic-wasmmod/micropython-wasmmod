# Forward to metalmod port (muscles live under extmod/metalmod; rename→metal later).
METALMOD_PORT := $(abspath ../../extmod/metalmod/port)

BOARD ?= X86_64_BIOS
ENGINE ?= mp
REPL ?= 0

.PHONY: all run clean
all run clean:
	$(MAKE) -C $(METALMOD_PORT) $@ BOARD=$(BOARD) ENGINE=$(ENGINE) REPL=$(REPL)
