TARGET?=x86_64-unknown-uefi
BUILD=build/$(TARGET)

SRC_DIR=.


$(BUILD)/boot.efi:  $(SRC_DIR)/Cargo.toml $(SRC_DIR)/src/* #$(SRC_DIR)/src/*/*
	mkdir -p $(BUILD)
	cargo rustc \
		-Z build-std=core,alloc \
		-Z build-std-features=compiler-builtins-mem \
		--target $(TARGET) \
		--bin uefi \
		--release \
		-- \
		-C soft-float \
		--emit link=$@

