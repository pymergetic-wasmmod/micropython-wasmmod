# ports/metal

Thin forward → `extmod/metalmod/port`.

```bash
# BIOS Multiboot + COM1 µPy smoke (all engines)
make -C ports/metal BOARD=X86_64_BIOS ENGINE=mp run
make -C ports/metal BOARD=X86_64_BIOS ENGINE=upy run
make -C ports/metal BOARD=X86_64_BIOS ENGINE=mpwm run

# Interactive REPL (no auto-exit smoke)
make -C ports/metal BOARD=X86_64_BIOS ENGINE=mp REPL=1

# UEFI OVMF + COM1 µPy smoke
make -C ports/metal BOARD=X86_64_UEFI ENGINE=mp run
```

Smoke prints `upy ok` then exits. Muscles stay under `extmod/metalmod/*` (rename → `metal` planned).
