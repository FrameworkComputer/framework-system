linux:
	mkdir -p out

	cargo build --release
	cp -f target/release/framework_tool out/dp_hdmi_version

	env FWK_DP_HDMI_BIN=dp-flash-008 cargo build --release
	cp target/release/framework_tool out/dp_update_008

	env FWK_DP_HDMI_BIN=dp-flash-100 cargo build --release
	cp target/release/framework_tool out/dp_update_100

	env FWK_DP_HDMI_BIN=dp-flash-101 cargo build --release
	cp target/release/framework_tool out/dp_update_101

	env FWK_DP_HDMI_BIN=hdmi-flash-006 cargo build --release
	cp target/release/framework_tool out/hdmi_update_006

	env FWK_DP_HDMI_BIN=hdmi-flash-105 cargo build --release
	cp target/release/framework_tool out/hdmi_update_105

windows:
	mkdir -p out

	cargo build --release
	cp -f target/release/framework_tool.exe out/dp_hdmi_version.exe

	env FWK_DP_HDMI_BIN=dp-flash-008 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/dp_update_008.exe

	env FWK_DP_HDMI_BIN=dp-flash-100 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/dp_update_100.exe

	env FWK_DP_HDMI_BIN=dp-flash-101 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/dp_update_101.exe

	env FWK_DP_HDMI_BIN=hdmi-flash-006 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/hdmi_update_006.exe

	env FWK_DP_HDMI_BIN=hdmi-flash-105 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/hdmi_update_105.exe
