TARGET?=x86_64-unknown-uefi
BUILD=build/$(TARGET)
FEATURES?=''

SRC_DIR=.

QEMU?=qemu-system-x86_64
OVMF_CODE?=$(BUILD)/OVMF_CODE.fd
OVMF_VARS?=$(BUILD)/OVMF_VARS.fd

QEMU_FLAGS=\
	-M q35 \
	-m 1024 \
	-net none \
	-vga std \
	-drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE) \
	-drive if=pflash,format=raw,file=$(BUILD)/ovmf_vars.fd

QEMU_TEST_FLAGS=\
	$(QEMU_FLAGS) \
	-display none \
	-serial file:$(BUILD)/serial.log \
	-no-reboot

.PHONY: qemu clean test test-qemu

all: $(BUILD)/boot.img

iso: $(BUILD)/UEFI-Shell-fwk.iso

clean:
	rm -rf $(BUILD)

qemu: $(BUILD)/boot.img $(BUILD)/OVMF_CODE.fd $(BUILD)/ovmf_vars.fd
	$(QEMU) $(QEMU_FLAGS) -drive format=raw,file=$<

# Create ESP partition and filesystem
$(BUILD)/boot.img: $(BUILD)/efi.img
	dd if=/dev/zero of=$@.tmp bs=512 count=100352
	parted $@.tmp -s -a minimal mklabel gpt
	parted $@.tmp -s -a minimal mkpart EFI FAT16 2048s 93716s
	parted $@.tmp -s -a minimal toggle 1 boot
	dd if=$< of=$@.tmp bs=512 count=98304 seek=2048 conv=notrunc
	mv $@.tmp $@

# Create filesystem with updater (bootx64.efi)
$(BUILD)/efi.img: $(BUILD)/boot.efi
	dd if=/dev/zero of=$@.tmp bs=512 count=98304
	mkfs.vfat $@.tmp
	mmd -i $@.tmp efi
	mmd -i $@.tmp efi/boot
	echo 'efi\boot\bootx64.efi --version' > startup.nsh
	mcopy -i $@.tmp startup.nsh ::efi/boot/startup.nsh
	rm -f startup.nsh
	mcopy -i $@.tmp $< ::efi/boot/bootx64.efi
	mv $@.tmp $@

$(BUILD)/shellx64.efi:
	mkdir -p $(BUILD)
	curl -L https://github.com/pbatard/UEFI-Shell/releases/download/24H2/shellx64.efi -o $@

# Download OVMF firmware for QEMU
$(BUILD)/OVMF_CODE.fd $(BUILD)/OVMF_VARS.fd:
	mkdir -p $(BUILD)
	curl -L https://retrage.github.io/edk2-nightly/bin/RELEASEX64_OVMF_CODE.fd -o $(BUILD)/OVMF_CODE.fd
	curl -L https://retrage.github.io/edk2-nightly/bin/RELEASEX64_OVMF_VARS.fd -o $(BUILD)/OVMF_VARS.fd

$(BUILD)/UEFI-Shell-fwk.iso: $(BUILD)/boot.efi $(BUILD)/shellx64.efi
	mkdir -p $(BUILD)/$@.tmp/efi/boot
	cp $(BUILD)/boot.efi $(BUILD)/$@.tmp/efi/boot/fwk.efi
	cp $(BUILD)/shellx64.efi $(BUILD)/$@.tmp/efi/boot/bootx64.efi
	genisoimage -v \
		-V "UEFI SHELL with fwk.efi" \
		-JR \
		-o "$(BUILD)/UEFI-Shell-fwk.iso" \
		$(BUILD)/$@.tmp

$(BUILD)/boot.efi: ../Cargo.lock $(SRC_DIR)/Cargo.toml $(SRC_DIR)/src/*
	  mkdir -p $(BUILD)
		cargo rustc \
		--target $(TARGET) \
		--features $(FEATURES) \
		--release \
		-- \
		--emit link=framework_uefi/$@

# Test targets
$(BUILD)/ovmf_vars.fd: $(BUILD)/OVMF_VARS.fd
	cp $< $@

$(BUILD)/test.img: $(BUILD)/boot.efi $(BUILD)/shellx64.efi tests/test.nsh winux.bin
	dd if=/dev/zero of=$@.tmp bs=512 count=131072
	mkfs.vfat $@.tmp
	mmd -i $@.tmp efi
	mmd -i $@.tmp efi/boot
	# Copy shell as boot loader, our tool as fwk.efi
	mcopy -i $@.tmp $(BUILD)/shellx64.efi ::efi/boot/bootx64.efi
	mcopy -i $@.tmp $(BUILD)/boot.efi ::efi/boot/fwk.efi
	# Also copy as bootx64 location for the test script
	mcopy -i $@.tmp $(BUILD)/boot.efi ::bootx64.efi
	# Copy test script as startup.nsh
	mcopy -i $@.tmp tests/test.nsh ::startup.nsh
	# Copy test files
	mcopy -i $@.tmp winux.bin ::winux.bin
	mv $@.tmp $@

$(BUILD)/test-boot.img: $(BUILD)/test.img
	dd if=/dev/zero of=$@.tmp bs=512 count=133120
	parted $@.tmp -s -a minimal mklabel gpt
	parted $@.tmp -s -a minimal mkpart EFI FAT16 2048s 131071s
	parted $@.tmp -s -a minimal toggle 1 boot
	dd if=$< of=$@.tmp bs=512 count=131072 seek=2048 conv=notrunc
	mv $@.tmp $@

test-qemu: $(BUILD)/test-boot.img $(BUILD)/OVMF_CODE.fd $(BUILD)/ovmf_vars.fd
	@echo "Running UEFI tests in QEMU..."
	@rm -f $(BUILD)/serial.log
	timeout 60 $(QEMU) $(QEMU_TEST_FLAGS) -drive format=raw,file=$< || true
	@echo ""
	@echo "=== Test Output ==="
	@cat $(BUILD)/serial.log
	@echo ""
	@echo "=== Test Results ==="
	@if grep -q "TESTS_COMPLETE" $(BUILD)/serial.log; then \
		echo "All tests completed."; \
	else \
		echo "ERROR: Tests did not complete!"; \
		exit 1; \
	fi
	@if grep -q "TEST_FAILED" $(BUILD)/serial.log; then \
		echo "ERROR: Some tests failed!"; \
		grep "TEST_FAILED" $(BUILD)/serial.log; \
		exit 1; \
	fi
	@echo "All tests passed!"

test: test-qemu
