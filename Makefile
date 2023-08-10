linux:
	rm -rf out
	mkdir -p out

	cargo build --release
	cp -f target/release/framework_tool out/dp_hdmi_version

	env FWK_DP_HDMI_BIN=dp-flash-006 cargo build --release
	cp target/release/framework_tool out/dp_update_006

	env FWK_DP_HDMI_BIN=dp-flash-008 cargo build --release
	cp target/release/framework_tool out/dp_update_008

	env FWK_DP_HDMI_BIN=dp-flash-100 cargo build --release
	cp target/release/framework_tool out/dp_update_100

	env FWK_DP_HDMI_BIN=dp-flash-101 cargo build --release
	cp target/release/framework_tool out/dp_update_101

	env FWK_DP_HDMI_BIN=hdmi-flash-005 cargo build --release
	cp target/release/framework_tool out/hdmi_update_005

	env FWK_DP_HDMI_BIN=hdmi-flash-006 cargo build --release
	cp target/release/framework_tool out/hdmi_update_006

	env FWK_DP_HDMI_BIN=hdmi-flash-102 cargo build --release
	cp target/release/framework_tool out/hdmi_update_102

	env FWK_DP_HDMI_BIN=hdmi-flash-103 cargo build --release
	cp target/release/framework_tool out/hdmi_update_103

	env FWK_DP_HDMI_BIN=hdmi-flash-104 cargo build --release
	cp target/release/framework_tool out/hdmi_update_104

	env FWK_DP_HDMI_BIN=hdmi-flash-105 cargo build --release
	cp target/release/framework_tool out/hdmi_update_105

	env FWK_DP_HDMI_BIN=hdmi-flash-106 cargo build --release
	cp target/release/framework_tool out/hdmi_update_106

	chmod +x out/*

windows:
	rm -rf out
	mkdir -p out

	cargo build --release --no-default-features --features "windows"
	cp -f target/release/framework_tool.exe out/dp_hdmi_version.exe

	env FWK_DP_HDMI_BIN=dp-flash-006 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/dp_update_006.exe

	env FWK_DP_HDMI_BIN=dp-flash-008 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/dp_update_008.exe

	env FWK_DP_HDMI_BIN=dp-flash-100 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/dp_update_100.exe

	env FWK_DP_HDMI_BIN=dp-flash-101 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/dp_update_101.exe

	env FWK_DP_HDMI_BIN=hdmi-flash-005 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/hdmi_update_005.exe

	env FWK_DP_HDMI_BIN=hdmi-flash-006 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/hdmi_update_006.exe

	env FWK_DP_HDMI_BIN=hdmi-flash-102 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/hdmi_update_102.exe

	env FWK_DP_HDMI_BIN=hdmi-flash-103 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/hdmi_update_103.exe

	env FWK_DP_HDMI_BIN=hdmi-flash-104 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/hdmi_update_104.exe

	env FWK_DP_HDMI_BIN=hdmi-flash-105 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/hdmi_update_105.exe

	env FWK_DP_HDMI_BIN=hdmi-flash-106 cargo build --release --no-default-features --features "windows"
	cp target/release/framework_tool.exe out/hdmi_update_106.exe
