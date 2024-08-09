TARGET?=x86_64-unknown-uefi
BUILD=build/$(TARGET)

SRC_DIR=.

# sudo pkg install qemu edk2-qemu-x64
QEMU?=qemu-system-x86_64
OVMF_FD?=/usr/local/share/edk2-qemu/QEMU_UEFI-x86_64.fd
QEMU_FLAGS=\
	-M q35 \
	-m 1024 \
	-net none \
	-vga std \
	-bios /usr/local/share/edk2-qemu/QEMU_UEFI-x86_64.fd

.PHONY: qemu clean

all: $(BUILD)/boot.img

$(BUILD)/boot.img: $(BUILD)/boot.efi
	mkdir -p temp/EFI/BOOT
	cp $(BUILD)/boot.efi temp/EFI/BOOT/bootx64.efi
	makefs -t msdos \
		-o fat_type=16 \
		-o sectors_per_cluster=1 \
		-s 10000k \
		$(BUILD)/boot.img temp
	rm -rf temp

clean:
	rm -r $(BUILD)

qemu: $(BUILD)/boot.img
	$(QEMU) $(QEMU_FLAGS) $<

$(BUILD)/boot.efi: ../Cargo.lock $(SRC_DIR)/Cargo.toml $(SRC_DIR)/src/*
	  mkdir -p $(BUILD)
		cargo rustc \
		--target $(TARGET) \
		--release \
		-- \
		-C soft-float \
		--emit link=framework_uefi/$@
