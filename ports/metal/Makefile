# Forward to metalmod — real port lives there.
METALMOD_PORT := $(abspath ../../extmod/metalmod/port)

.PHONY: all run clean
all run clean:
	$(MAKE) -C $(METALMOD_PORT) $@ BOARD=$(BOARD)
