TARGET?=x86_64-unknown-uefi
BUILD=build/$(TARGET)
FEATURES?=''

SRC_DIR=.

QEMU?=qemu-system-x86_64
QEMU_FLAGS=\
	-M q35 \
	-m 1024 \
	-net none \
	-vga std \
	-device qemu-xhci,id=xhci \
	-device usb-host,vendorid=0x31e3,productid=0x1402 \
	-bios /usr/share/OVMF/OVMF_CODE.fd

.PHONY: qemu clean

all: $(BUILD)/boot.img

iso: $(BUILD)/UEFI-Shell-fwk.iso

clean:
	rm -rf $(BUILD)

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
	echo 'efi\boot\bootx64.efi --version' > startup.nsh
	mcopy -i $@.tmp startup.nsh ::efi/boot/startup.nsh
	rm -f startup.nsh
	mcopy -i $@.tmp $< ::efi/boot/bootx64.efi
	mv $@.tmp $@

$(BUILD)/shellx64.efi:
	wget https://github.com/pbatard/UEFI-Shell/releases/download/24H2/shellx64.efi -O $@

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
