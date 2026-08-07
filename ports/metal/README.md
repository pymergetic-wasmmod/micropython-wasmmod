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

Smoke serial markers: `floor ok` (TLSF mem + async), `wamr ok` when `ENGINE=mp|mpwm`, then `upy ok`, then `qemu ok` (BIOS) / `ovmf ok` (UEFI).

Host-only floor (no QEMU): `make -C extmod/metalmod/async`.

Freestanding WAMR (wasmmod OWN recipe + Metal platform GLUE) links for `ENGINE=mp|mpwm` on BIOS.

Muscles stay under `extmod/metalmod/*` (rename → `metal` planned).
