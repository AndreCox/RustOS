<div align="center">

# RustOS

**A small x86_64 operating system written in Rust that grew from an OS-dev experiment into a kernel capable of running DOOM and Quake.**

[![Rust](https://img.shields.io/badge/Rust-Nightly-f74c00?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Architecture](https://img.shields.io/badge/Architecture-x86__64-2f6feb?style=for-the-badge)](https://wiki.osdev.org/X86-64)
[![Bootloader](https://img.shields.io/badge/Bootloader-Limine-5c3ee8?style=for-the-badge)](https://github.com/limine-bootloader/limine)
[![Kernel](https://img.shields.io/badge/Kernel-no__std-111111?style=for-the-badge)](https://doc.rust-lang.org/core/)
[![Blog](https://img.shields.io/badge/Read%20the%20Blog-andrecox.io-0a7f5a?style=for-the-badge)](https://andrecox.io/work/rust-os)

[Blog Post](https://andrecox.io/work/rust-os) • [Build](#build-and-run) • [Highlights](#highlights) • [AI Use Disclosure](#ai-use-disclosure)

</div>

## Overview

RustOS is a `no_std` hobby operating system for `x86_64` built from scratch in Rust. It uses the Limine bootloader and includes the core pieces you would expect from a tiny kernel: interrupts, memory management, preemptive multitasking, a FAT32 filesystem, a syscall layer, and userspace programs.

The project started as a way to learn low-level systems programming and turned into something much more complete and much more fun: a custom OS that can boot, schedule tasks, load programs, and run native ports of DOOM and Quake.

## Demo Video

<div align="center">

<a href="https://youtu.be/oWDV8jlYdrE">
  <img src="https://img.youtube.com/vi/oWDV8jlYdrE/maxresdefault.jpg" alt="RustOS demo video thumbnail" width="720" />
</a>

**[Watch RustOS on YouTube](https://youtu.be/oWDV8jlYdrE)**

</div>

## Highlights

| Area         | What it includes                                           |
| ------------ | ---------------------------------------------------------- |
| Kernel       | Rust `no_std` kernel targeting `x86_64`                    |
| Boot         | Limine boot flow and custom kernel image                   |
| CPU setup    | GDT, IDT, interrupts, and timer-driven scheduling          |
| Memory       | Physical frame allocation, paging, and dynamic heap growth |
| Multitasking | Preemptive round-robin scheduler with idle task support    |
| Filesystem   | FAT32 support for loading apps and saving files            |
| Userspace    | Syscalls for display, input, timing, and filesystem access |
| Fun part     | Native ports of DOOM and Quake                             |

## Build and Run

RustOS uses nightly Rust and a Makefile-based workflow.

```bash
make run
```

Useful targets:

- `make` builds the kernel, apps, and bootable ISO
- `make run` boots the OS in QEMU
- `make debug` starts QEMU with `rust-gdb`

Local tools you will need include QEMU, `xorriso`, `mkfs.fat`, and `mtools`.

## Why This Project Exists

The goal was to understand how kernels are built from the ground up while making something fun enough to stay motivating. That meant not stopping at a boot screen: the project kept growing until it supported graphics, input, files, multitasking, and real software.

## Read More

The full write-up with implementation details, screenshots, and the story behind the project lives here:

**[andrecox.io/work/rust-os](https://andrecox.io/work/rust-os)**

## AI Use Disclosure

AI was used to help with:

- Porting C code to Rust and generating wrappers for DOOM and Quake
- Debugging deadlocks and other low-level issues during development
- Generating some boilerplate for kernel and driver code
- AI autocompletion with Copilot during regular development
- Refactoring code for readability and maintainability
- Finding and removing dead code
- General research and learning around OS development concepts and best practices
