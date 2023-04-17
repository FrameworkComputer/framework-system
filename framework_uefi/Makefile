TARGET?=x86_64-unknown-uefi
BUILD=build/$(TARGET)

SRC_DIR=.

QEMU?=qemu-system-x86_64
QEMU_FLAGS=\
	-M q35 \
	-m 1024 \
	-net none \
	-vga std \
	-bios /usr/share/OVMF/OVMF_CODE.fd

.PHONY: qemu clean

all: $(BUILD)/boot.img

clean:
	rm -r $(BUILD)

qemu: $(BUILD)/boot.img
	$(QEMU) $(QEMU_FLAGS) $<

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
	mcopy -i $@.tmp $< ::efi/boot/bootx64.efi
	mv $@.tmp $@

$(BUILD)/boot.efi: ../Cargo.lock $(SRC_DIR)/Cargo.toml $(SRC_DIR)/src/*
	  mkdir -p $(BUILD)
		cargo rustc \
		--target $(TARGET) \
		--release \
		-- \
		-C soft-float \
		--emit link=framework_uefi/$@
