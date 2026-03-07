{
  description = "Framework System library and CLI tool for Framework Computer hardware";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        # Read toolchain from rust-toolchain.toml
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Toolchain extended with Windows cross-compilation target
        rustToolchainWindows = rustToolchain.override {
          targets = [ "x86_64-pc-windows-gnu" ];
        };

        # Create a custom rustPlatform with our toolchain
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        # rustPlatform with Windows cross-compilation target
        rustPlatformWindows = pkgs.makeRustPlatform {
          cargo = rustToolchainWindows;
          rustc = rustToolchainWindows;
        };

        # Common build inputs for OS builds
        commonBuildInputs = with pkgs; [
          openssl
          libgit2
          zlib
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          systemdLibs # libudev
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.libiconv
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
        ];

        commonNativeBuildInputs = with pkgs; [
          pkg-config
          zlib # Required by framework_lib build script at runtime
        ];

        # Filter source to only include files needed for the build
        buildSrc = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            let
              baseName = baseNameOf path;
              relativePath = pkgs.lib.removePrefix (toString ./. + "/") path;
              # Only include files/folders needed for the Rust build
              includedRoots = [
                "framework_lib"
                "framework_tool"
                "framework_uefi"
                "res"
                ".cargo"
              ];
              includedFiles = [
                "Cargo.toml"
                "Cargo.lock"
                "rust-toolchain.toml"
              ];
              isIncludedRoot = builtins.any (root:
                relativePath == root || pkgs.lib.hasPrefix (root + "/") relativePath
              ) includedRoots;
            in
            isIncludedRoot || builtins.elem baseName includedFiles;
        };

        # Git dependency output hashes
        gitDependencyHashes = {
          "smbios-lib-0.9.1" = "sha256-3L8JaA75j9Aaqg1z9lVs61m6CvXDeQprEFRq+UDCHQo=";
        };

        # Build function for the CLI tool (Linux/macOS)
        buildFrameworkTool = { release ? false, features ? [] }:
          let
            profile = if release then "release" else "debug";
            featuresStr = if features == [] then "" else "--features ${builtins.concatStringsSep "," features}";
          in
          rustPlatform.buildRustPackage {
            pname = "framework_tool";
            version = "0.5.0";

            src = buildSrc;

            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = gitDependencyHashes;
            };

            buildType = profile;

            # Build only the tool, not the UEFI package
            buildPhase = ''
              runHook preBuild
              cargo build \
                ${if release then "--release" else ""} \
                -p framework_tool \
                ${featuresStr}
              runHook postBuild
            '';

            # Run tests for framework_lib
            checkPhase = ''
              runHook preCheck
              cargo test -p framework_lib
              runHook postCheck
            '';

            installPhase = ''
              runHook preInstall
              mkdir -p $out/bin
              cp target/${profile}/framework_tool $out/bin/
              runHook postInstall
            '';

            nativeBuildInputs = commonNativeBuildInputs;
            buildInputs = commonBuildInputs;

            # Environment variables for C library bindings
            OPENSSL_NO_VENDOR = "1";
            LIBGIT2_NO_VENDOR = "1";
          };

        # MinGW cross-compiler for Windows builds
        mingw = pkgs.pkgsCross.mingwW64.stdenv.cc;
        mingwPthreads = pkgs.pkgsCross.mingwW64.windows.pthreads;

        # Build function for Windows cross-compilation (Linux -> Windows)
        buildFrameworkToolWindows = { release ? false }:
          let
            profile = if release then "release" else "debug";
          in
          rustPlatformWindows.buildRustPackage {
            pname = "framework_tool";
            version = "0.5.0";

            src = buildSrc;

            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = gitDependencyHashes;
            };

            buildType = profile;
            buildNoDefaultFeatures = true;

            # Disable cargo-auditable as it's incompatible with cross-compilation
            auditable = false;

            buildPhase = ''
              runHook preBuild
              cargo build \
                ${if release then "--release" else ""} \
                --target x86_64-pc-windows-gnu \
                -p framework_tool
              runHook postBuild
            '';

            # Skip check phase - can't run .exe on Linux
            doCheck = false;

            installPhase = ''
              runHook preInstall
              mkdir -p $out/bin
              cp target/x86_64-pc-windows-gnu/${profile}/framework_tool.exe $out/bin/
              runHook postInstall
            '';

            nativeBuildInputs = [ mingw ];

            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "${mingw}/bin/x86_64-w64-mingw32-gcc";
            CC_x86_64_pc_windows_gnu = "${mingw}/bin/x86_64-w64-mingw32-gcc";
            CXX_x86_64_pc_windows_gnu = "${mingw}/bin/x86_64-w64-mingw32-g++";
            AR_x86_64_pc_windows_gnu = "${mingw}/bin/x86_64-w64-mingw32-ar";
            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = "-L native=${mingwPthreads}/lib";
          };

        # Build function for UEFI application
        buildFrameworkUefi = { release ? false, features ? [] }:
          let
            profile = if release then "release" else "debug";
            featuresStr = if features == [] then "" else "--features ${builtins.concatStringsSep "," features}";
          in
          rustPlatform.buildRustPackage {
            pname = "framework_uefi";
            version = "0.5.0";

            src = buildSrc;

            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = gitDependencyHashes;
            };

            buildType = profile;
            buildNoDefaultFeatures = true;

            # Disable cargo-auditable as it's incompatible with UEFI linker
            auditable = false;

            # Target for UEFI - passed via args to avoid affecting build scripts
            buildPhase = ''
              runHook preBuild
              cargo build \
                ${if release then "--release" else ""} \
                --target x86_64-unknown-uefi \
                -p framework_uefi \
                ${featuresStr}
              runHook postBuild
            '';

            # Skip check phase - UEFI binaries can't be tested on host
            doCheck = false;

            installPhase = ''
              runHook preInstall
              mkdir -p $out/bin
              cp target/x86_64-unknown-uefi/${profile}/uefitool.efi $out/bin/
              runHook postInstall
            '';

            nativeBuildInputs = commonNativeBuildInputs;
            buildInputs = commonBuildInputs;

            # Environment variables for C library bindings
            OPENSSL_NO_VENDOR = "1";
            LIBGIT2_NO_VENDOR = "1";
          };

        # Package definitions
        framework-tool-debug = buildFrameworkTool { release = false; };
        framework-tool-release = buildFrameworkTool { release = true; };
        framework-uefi-debug = buildFrameworkUefi { release = false; };
        framework-uefi-release = buildFrameworkUefi { release = true; };
        framework-tool-windows = buildFrameworkToolWindows { release = true; };
        framework-tool-windows-debug = buildFrameworkToolWindows { release = false; };

        # Wrapper script to run the UEFI build in an emulator
        run-qemu = pkgs.writeShellScriptBin "run-framework-uefi-qemu" ''
          set -e

          # Create temporary directory for ESP and OVMF vars
          TMPDIR=$(mktemp -d)
          trap "rm -rf $TMPDIR" EXIT

          # Set up ESP filesystem structure
          mkdir -p "$TMPDIR/esp/efi/boot"
          cp ${framework-uefi-debug}/bin/uefitool.efi "$TMPDIR/esp/efi/boot/bootx64.efi"

          # Copy OVMF_VARS.fd to temp (it needs to be writable)
          cp ${pkgs.OVMF.fd}/FV/OVMF_VARS.fd "$TMPDIR/OVMF_VARS.fd"
          chmod 644 "$TMPDIR/OVMF_VARS.fd"

          echo "Starting QEMU with Framework UEFI tool..."
          echo "Debug output will be written to: $TMPDIR/debug.log"

          ${pkgs.qemu}/bin/qemu-system-x86_64 \
            ''${QEMU_KVM:+-enable-kvm} \
            -M q35 \
            -m 1024 \
            -net none \
            -serial stdio \
            -debugcon "file:$TMPDIR/debug.log" -global isa-debugcon.iobase=0x402 \
            -usb \
            -drive if=pflash,format=raw,readonly=on,file=${pkgs.OVMF.fd}/FV/OVMF_CODE.fd \
            -drive if=pflash,format=raw,readonly=off,file="$TMPDIR/OVMF_VARS.fd" \
            -drive format=raw,file=fat:rw:"$TMPDIR/esp" \
            "$@"
        '';

        # Wrapper for release QEMU build
        run-qemu-release = pkgs.writeShellScriptBin "run-framework-uefi-qemu" ''
          set -e

          TMPDIR=$(mktemp -d)
          trap "rm -rf $TMPDIR" EXIT

          mkdir -p "$TMPDIR/esp/efi/boot"
          cp ${framework-uefi-release}/bin/uefitool.efi "$TMPDIR/esp/efi/boot/bootx64.efi"

          cp ${pkgs.OVMF.fd}/FV/OVMF_VARS.fd "$TMPDIR/OVMF_VARS.fd"
          chmod 644 "$TMPDIR/OVMF_VARS.fd"

          echo "Starting QEMU with Framework UEFI tool (release)..."

          ${pkgs.qemu}/bin/qemu-system-x86_64 \
            ''${QEMU_KVM:+-enable-kvm} \
            -M q35 \
            -m 1024 \
            -net none \
            -serial stdio \
            -debugcon "file:$TMPDIR/debug.log" -global isa-debugcon.iobase=0x402 \
            -usb \
            -drive if=pflash,format=raw,readonly=on,file=${pkgs.OVMF.fd}/FV/OVMF_CODE.fd \
            -drive if=pflash,format=raw,readonly=off,file="$TMPDIR/OVMF_VARS.fd" \
            -drive format=raw,file=fat:rw:"$TMPDIR/esp" \
            "$@"
        '';

      in
      {
        checks = {
          inherit framework-tool-release framework-uefi-release;
        };

        packages = {
          default = framework-tool-release;
          tool = framework-tool-release;
          tool-debug = framework-tool-debug;
          uefi = framework-uefi-release;
          uefi-debug = framework-uefi-debug;
          windows = framework-tool-windows;
          windows-debug = framework-tool-windows-debug;
          run-qemu = run-qemu;
          run-qemu-release = run-qemu-release;
        };

        # Convenience apps for `nix run`
        apps = {
          default = flake-utils.lib.mkApp { drv = framework-tool-release; exePath = "/bin/framework_tool"; };
          tool = flake-utils.lib.mkApp { drv = framework-tool-release; exePath = "/bin/framework_tool"; };
          qemu = flake-utils.lib.mkApp { drv = run-qemu; };
          qemu-release = flake-utils.lib.mkApp { drv = run-qemu-release; };
        };

        devShells.cross-windows = pkgs.mkShell {
          packages = [
            rustToolchainWindows
          ];

          # Ensure build scripts (e.g. libgit2-sys) use the native host compiler
          HOST_CC = "cc";

          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "${mingw}/bin/x86_64-w64-mingw32-gcc";
          CC_x86_64_pc_windows_gnu = "${mingw}/bin/x86_64-w64-mingw32-gcc";
          CXX_x86_64_pc_windows_gnu = "${mingw}/bin/x86_64-w64-mingw32-g++";
          AR_x86_64_pc_windows_gnu = "${mingw}/bin/x86_64-w64-mingw32-ar";
          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = "-L native=${mingwPthreads}/lib";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain

            # Development tools
            gnumake
            qemu
            parted
            OVMF

            # Build dependencies
            pkg-config
            openssl
            libgit2
            zlib
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            systemdLibs # libudev
          ];

          OPENSSL_NO_VENDOR = "1";
          LIBGIT2_NO_VENDOR = "1";

          # Set up OVMF symlinks for `make run` compatibility
          shellHook = ''
            if [ ! -d ovmf ] || [ ! -e ovmf/OVMF_CODE.fd ] || [ ! -e ovmf/OVMF_VARS.fd ]; then
              mkdir -p ovmf
              ln -sf ${pkgs.OVMF.fd}/FV/OVMF_CODE.fd ovmf/OVMF_CODE.fd
              # OVMF_VARS needs to be writable, so copy it instead of symlinking
              cp -f ${pkgs.OVMF.fd}/FV/OVMF_VARS.fd ovmf/OVMF_VARS.fd
              chmod 644 ovmf/OVMF_VARS.fd
              echo "OVMF files set up in ovmf/"
            fi
          '';
        };
      }
    );
}
