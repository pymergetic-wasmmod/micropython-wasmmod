# Forward to metal port (muscles under extmod/metal).
METAL_PORT := $(abspath ../../extmod/metal/port)

BOARD ?= X86_64_BIOS
ENGINE ?= mp
REPL ?= 0
LIVE ?= 0
LIVE_SSH ?= 0

.PHONY: all run clean live-http live-ssh
all run clean:
	$(MAKE) -C $(METAL_PORT) $@ BOARD=$(BOARD) ENGINE=$(ENGINE) REPL=$(REPL) LIVE=$(LIVE) LIVE_SSH=$(LIVE_SSH)

live-http:
	$(MAKE) -C $(METAL_PORT) live-http BOARD=$(BOARD) ENGINE=$(ENGINE) REPL=0 LIVE=1 LIVE_SSH=0

live-ssh:
	$(MAKE) -C $(METAL_PORT) live-ssh BOARD=$(BOARD) ENGINE=$(ENGINE) REPL=0 LIVE=0 LIVE_SSH=1
