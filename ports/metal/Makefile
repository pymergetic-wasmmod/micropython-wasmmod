# Forward to metal port (muscles under extmod/metal).
METAL_PORT := $(abspath ../../extmod/metal/port)

BOARD ?= X86_64_BIOS
ENGINE ?= mp
REPL ?= 0
LIVE ?= 0
LIVE_SSH ?= 0
SMP ?= 2

.PHONY: all run clean live-http live-ssh repl
all run clean:
	$(MAKE) -C $(METAL_PORT) $@ BOARD=$(BOARD) ENGINE=$(ENGINE) REPL=$(REPL) LIVE=$(LIVE) LIVE_SSH=$(LIVE_SSH) SMP=$(SMP)

repl:
	$(MAKE) -C $(METAL_PORT) repl BOARD=$(BOARD) ENGINE=$(ENGINE) SMP=$(SMP)

live-http:
	$(MAKE) -C $(METAL_PORT) live-http BOARD=$(BOARD) ENGINE=$(ENGINE) REPL=0 LIVE=1 LIVE_SSH=0 SMP=$(SMP)

live-ssh:
	$(MAKE) -C $(METAL_PORT) live-ssh BOARD=$(BOARD) ENGINE=$(ENGINE) REPL=0 LIVE=0 LIVE_SSH=1 SMP=$(SMP)
