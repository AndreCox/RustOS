# Variables
KERNEL := target/x86_64-kernel/release/RustOS
ISO := RustOS.iso
ISO_ROOT := iso_root

RUST_SOURCES := $(shell find src -name '*.rs' 2>/dev/null)

.PHONY: all clean run iso

all: $(ISO)

# 1. Build the Rust kernel
$(KERNEL): $(RUST_SOURCES) Cargo.toml Cargo.lock x86_64-kernel.json
	@echo "==> Compiling Rust Kernel"
	cargo +nightly build --release --target x86_64-kernel.json -Zbuild-std=core,compiler_builtins,alloc -Zbuild-std-features=compiler-builtins-mem

# 2. Setup iso_root and build the ISO
$(ISO): $(KERNEL) limine.conf limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin limine/limine
	@echo "==> Preparing ISO Root"
	mkdir -p $(ISO_ROOT)
	cp $(KERNEL) $(ISO_ROOT)/kernel.elf
	cp limine.conf $(ISO_ROOT)/
	cp limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin $(ISO_ROOT)/
	@echo "==> Creating ISO"
	xorriso -as mkisofs -b limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		$(ISO_ROOT) -o $(ISO)
	@echo "==> Deploying Limine"
	./limine/limine bios-install $(ISO)

# Build limine if its output files are missing
limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin limine/limine:
	@echo "==> Ensuring Limine is built"
	@if [ ! -d limine ]; then echo "limine/ directory not found"; false; fi
	@cd limine && make

# 3. Clean up
clean:
	rm -rf $(ISO_ROOT) $(ISO)
	cargo clean

# 4. Shortcut to build and run in QEMU
run: $(ISO)
	qemu-system-x86_64 -cdrom $(ISO) -m 512M -serial stdio