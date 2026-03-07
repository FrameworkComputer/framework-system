@echo -off
echo "=== UEFI Test Suite Starting ==="

# Clean up any files from previous test runs
if exist fs0:\dump.bmp then
    del fs0:\dump.bmp
endif

# Set path to our tool (fwk.efi is our framework tool)
set fwk fs0:\efi\boot\fwk.efi

echo ""
echo "=== TEST: --version ==="
%fwk% --version
if %lasterror% == 0 then
    echo "TEST_PASSED: version"
else
    echo "TEST_FAILED: version (exit code %lasterror%)"
endif

echo ""
echo "=== TEST: --help ==="
%fwk% --help
# --help returns 1 (LOAD_ERROR) which is expected behavior for this tool
if %lasterror% == 0x1 then
    echo "TEST_PASSED: help"
else
    echo "TEST_FAILED: help (expected 0x1, got %lasterror%)"
endif

echo ""
echo "=== TEST: --hash on tool itself ==="
%fwk% --hash fs0:\efi\boot\fwk.efi
if %lasterror% == 0 then
    echo "TEST_PASSED: hash"
else
    echo "TEST_FAILED: hash (exit code %lasterror%)"
endif

echo ""
echo "=== TEST: --hash on winux.bin ==="
%fwk% --hash fs0:\winux.bin
if %lasterror% == 0 then
    echo "TEST_PASSED: hash_winux"
else
    echo "TEST_FAILED: hash_winux (exit code %lasterror%)"
endif

echo ""
echo "=== TEST: --capsule dump ==="
%fwk% --capsule fs0:\winux.bin --dump fs0:\dump.bmp
if %lasterror% == 0 then
    echo "TEST_PASSED: capsule_dump"
else
    echo "TEST_FAILED: capsule_dump (exit code %lasterror%)"
endif

echo ""
echo "=== TEST: verify dump.bmp was created ==="
if exist fs0:\dump.bmp then
    %fwk% --hash fs0:\dump.bmp
    if %lasterror% == 0 then
        echo "TEST_PASSED: dump_exists"
    else
        echo "TEST_FAILED: dump_exists (hash command failed)"
    endif
else
    echo "TEST_FAILED: dump_exists (dump.bmp was not created)"
endif

echo ""
echo "=== UEFI Test Suite Complete ==="
echo "TESTS_COMPLETE"

# Shutdown QEMU
reset -s
