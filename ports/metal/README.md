# ports/metal

Thin forward → `extmod/metal/port`.

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

Build dirs live under `extmod/metal/port/build/` per-`ENGINE` (`build/$(BOARD)-$(ENGINE)…`) so `LINK_WAMR` / TOP never stale-collide.

## µPy modules (Metal port)

| Module | Role |
|--------|------|
| `network.LAN` | virtio-net AbstractNIC (`ifconfig` / `resolve` / …) |
| `socket` | TCP/UDP via AbstractNIC |
| `framebuf` | soft RGB565 framebuffer helpers |
| `pymergetic.metal.net.ssh` | nested builtin (`pm_metal_net_ssh_*`) |

IDE stubs: `extmod/metal/typings/` (pyright `stubPath`).

## Smoke serial markers (order)

| Marker | Meaning |
|--------|---------|
| `console ok` | ring + UART attach (+ history replay) |
| `floor ok` | TLSF + cooperative async sleep/yield |
| `net ok` | virtio-net PCI + RX/TX vrings + ARP TX |
| `dhcp ok` | DORA lease from QEMU user-net DHCP |
| `ip ok` | mini IPv4 after DHCP + ARP announce |
| `ping ok` | ICMP echo reply from QEMU gateway 10.0.2.2 |
| `udp`/`dns`/`tcp`/`http` ok | UDP, real DNS, TCP, HTTP server |
| `ssh stub` | SSH not implemented; smoke continues |
| `http client ok` | outbound TCP connect + HTTP GET example.com |
| `ntp ok` | NTP client query time.google.com → sane Unix time |
| `tftp ok` | TFTP RRQ metal.txt from QEMU user-net TFTP |
| `draw ok` | soft DrawSurface RGB565 + 8×8 glyphs |
| `vt ok` | F1–F6 cell mux + soft render; live console→VT fan-out |
| `tui ok` | F7 DOS-Edit dashboard; network pane + live faces |
| `kbd ok` | F1–F7 scancodes switch active VT |
| `wamr ok` | freestanding WAMR init (`ENGINE=mp\|mpwm`) |
| `framebuf ok` | `MICROPY_PY_FRAMEBUF` smoke |
| `network ok` | `network.LAN` ifconfig/isconnected after DHCP |
| `dns py ok` | `network.LAN.resolve` (literal + real DNS A) |
| `socket ok` | `socket` connect/send/recv HTTP GET via AbstractNIC |
| `ssh py ok` / `ssh stub` | µPy `ssh.available()` true/false |
| `upy ok` | µPy `print` |
| `qemu ok` / `ovmf ok` | board exit success |

External QEMU user-net smokes (`dns`, `http client`, `ntp`) retry up to 3× with a short `ip_poll` settle between attempts. µPy follow-ons mirror that pattern for `dns py` and `socket`.

QEMU adds `-netdev user,id=n0 -device virtio-net-pci,netdev=n0` for net.

Host-only floor (no QEMU): `make -C extmod/metal/async`.

Freestanding WAMR (wasmmod OWN recipe + Metal platform GLUE) links for `ENGINE=mp|mpwm`.

Muscles live under `extmod/metal/{mem,async,console,draw,bus,dev,net,shell,…}`.

## Live HTTP / SSH

```bash
make -C ports/metal BOARD=X86_64_BIOS ENGINE=mp live-http
make -C ports/metal BOARD=X86_64_UEFI ENGINE=mp live-http
# Expect X86_64_{BIOS,UEFI}_LIVE_HTTP_OK — curls http://127.0.0.1:18080/ → metal ok

make -C ports/metal BOARD=X86_64_BIOS ENGINE=mp live-ssh
make -C ports/metal BOARD=X86_64_UEFI ENGINE=mp live-ssh
# Expect X86_64_{BIOS,UEFI}_LIVE_SSH_OK when guest prints live ssh on serial.
# Optional: host nc 127.0.0.1:22022 sees SSH-2.0-metal ident banner.
```
