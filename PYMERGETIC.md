# Pymergetic fleet

| Pill | Repo | Role |
|------|------|------|
| **upy** | [micropython/micropython](https://github.com/micropython/micropython) | vanilla µPy. CDN engine only. |
| **upywm** | [pymergetic-wasmmod/micropython-wasmmod](https://github.com/pymergetic-wasmmod/micropython-wasmmod) | upy + wasmmod. No metal. Old pill: mpwm. |
| **mp** | [pymergetic/metalpython](https://github.com/pymergetic/metalpython) | upywm + metal. Ancestor: upywm. |
| **wasmmod** | [pymergetic-wasmmod/wasmmod](https://github.com/pymergetic-wasmmod/wasmmod) | packs, loader, io, cdn client, `util.mem`, gen. Nested at `extmod/wasmmod`. |
| **metal** | [pymergetic/metal](https://github.com/pymergetic/metal) | `pymergetic.metal` on mp (`extmod/metal`). **`main`** = extmod cards. **`preview`** = standalone CMake runtime (doom). |
| **cdn** | [pymergetic-wasmmod/wasmmod-cdn](https://github.com/pymergetic-wasmmod/wasmmod-cdn) | catalog / inspect / publish / browser shells. Does not own the runtime. |
| **doom** | [pymergetic/metal-doom](https://github.com/pymergetic/metal-doom) | gfx proof against metal **preview**. |
| **os-sdk** | [pymergetic/os-sdk](https://github.com/pymergetic/os-sdk) | pin hub. Seats map: `packages/REPO_LAYOUT.md`. |

**This tree is upywm** — MicroPython + wasmmod. No `extmod/metal`.

Bump: `wasmmod → upywm → metal → metalpython → wasmmod-cdn → os-sdk`
