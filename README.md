# ros

A tiny x86_64 hobby kernel written in Rust.

## Why this exists

This is a fun side project for learning how operating systems work from the
ground up. I'm working through Philipp Oppermann's excellent
[Writing an OS in Rust](https://os.phil-opp.com/) series, with a couple of
twists:

- **Modernizing as I go.** The blog targets older tooling, so this repo uses
  `bootloader` 0.11 (BIOS + UEFI disk images, framebuffer instead of VGA text
  mode), the built-in `x86_64-unknown-none` target, and a custom QEMU runner in
  place of `bootimage`.
- **Writing my own code where I can.** Instead of pulling in crates for
  everything, I implement pieces myself to actually understand them — e.g. my
  own `RawSpinLock`, `SpinLock`, and interrupt-disabling `IrqSpinLock` in
  `kernel/src/sync/` instead of `spin::Mutex`, and no `lazy_static`/`volatile`.
- **Learning with an AI pair.** I've used Claude as a tutor along the way to
  explain concepts and help with modernization and test scaffolding.

Nothing here is meant to be production-grade — it's for learning and fun.

### Current progress

- Framebuffer text output and serial (UART 16550) logging
- GDT/TSS setup with a dedicated double-fault stack
- IDT with breakpoint, double-fault, and timer interrupt handlers (8259 PIC)
- Interrupt-safe spinlocks protecting the framebuffer and serial port
- In-kernel test framework with unit tests and integration tests
  (`basic_boot`, `should_panic`, `stack_overflow`)

## Layout

```
kernel/   the kernel itself (no_std, built for x86_64-unknown-none)
runner/   host-side tool: wraps the kernel ELF in a bootable disk image and launches QEMU
scripts/  ros-runner shim that builds the runner and invokes it (used by cargo as the target runner)
```

## Prerequisites

- [rustup](https://rustup.rs/) — `rust-toolchain.toml` pins nightly and installs
  the `x86_64-unknown-none` target, `rust-src`, and `llvm-tools-preview`
  automatically.
- QEMU (`qemu-system-x86_64` on your `PATH`), e.g. `brew install qemu`.

## Commands

Three cargo aliases are defined in `.cargo/config.toml`. They exist because the
workspace's default member is the host-side runner, so a bare `cargo build`
builds for your machine — the aliases select the kernel package and the bare-metal
target for you.

### `cargo kbuild`

```sh
cargo kbuild
```

Expands to `cargo build -p kernel --target x86_64-unknown-none`. Compiles the
kernel to an ELF binary under `target/x86_64-unknown-none/debug/kernel`. No
QEMU involved — handy for checking that things compile.

### `cargo krun`

```sh
cargo krun            # boot via BIOS
cargo krun -- uefi    # boot via UEFI (downloads OVMF firmware on first use)
```

Expands to `cargo run -p kernel --target x86_64-unknown-none`. Cargo builds the
kernel, then hands the ELF to `scripts/ros-runner` (configured as the target's
`runner`). The runner builds a bootable disk image with the `bootloader` crate
and launches QEMU with a window for the framebuffer and serial output piped to
your terminal. Close the QEMU window (or press `Ctrl-a x`) to exit.

### `cargo ktest`

```sh
cargo ktest
cargo ktest --test stack_overflow   # run a single integration test
```

Expands to `cargo test -q -p kernel --target x86_64-unknown-none`. Each test
binary is booted in a headless QEMU instance. Tests report results over serial
and signal pass/fail by writing to QEMU's `isa-debug-exit` device, which the
runner translates into a normal process exit code so `cargo test` sees success
or failure. Each test has a 5-minute timeout.
