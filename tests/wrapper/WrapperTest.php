<?php

/**
 * Test logic extracted from phanalist wrapper script
 */
function test_get_target($os, $arch) {
    // Current logic in phanalist
    if (strpos(strtoupper($os), 'DARWIN') !== false) {
        if ($arch === 'arm64' || $arch === 'aarch64') {
            return 'aarch64-apple-darwin';
        }
        return 'x86_64-apple-darwin';
    }

    if (strpos(strtoupper($os), 'WIN') !== false) {
        return 'x86_64-pc-windows-msvc';
    }

    if (strpos(strtoupper($os), 'LINUX') !== false) {
        // Mocking ldd check for now
        $libc = 'gnu'; 

        if ($arch === 'arm64' || $arch === 'aarch64') {
            return "aarch64-unknown-linux-$libc";
        }
        return "x86_64-unknown-linux-$libc";
    }

    return null;
}

function test_bin_path_resolution($target, $binDir, $releaseDir, $executableName, $binExists, $releaseExists) {
    $binPath = "$binDir/$target/$executableName";
    
    // Logic from phanalist fix
    if (!$binExists) {
        $releasePath = "$releaseDir/$target/$executableName";
        if ($releaseExists) {
            $binPath = $releasePath;
        }
    }
    
    return $binPath;
}

// Assertions
$errors = [];

function assert_equals($expected, $actual, $message) {
    global $errors;
    if ($expected !== $actual) {
        $errors[] = "FAIL: $message. Expected '$expected', got '$actual'.";
    } else {
        echo "PASS: $message" . PHP_EOL;
    }
}

// 1. Test OS Detection (The Darwin/Win bug)
assert_equals('aarch64-apple-darwin', test_get_target('Darwin', 'arm64'), "Should detect macOS ARM64 correctly");
assert_equals('x86_64-apple-darwin', test_get_target('Darwin', 'x86_64'), "Should detect macOS x86_64 correctly");
assert_equals('x86_64-pc-windows-msvc', test_get_target('Windows', 'x86_64'), "Should detect Windows x86_64 correctly");
assert_equals('x86_64-unknown-linux-gnu', test_get_target('Linux', 'x86_64'), "Should detect Linux x86_64 correctly");

// 2. Test Path Resolution (The release/ folder bug)
$binDir = "/path/to/bin";
$releaseDir = "/path/to/release";
$exe = "phanalist";
$target = "aarch64-apple-darwin";

assert_equals(
    "$binDir/$target/$exe",
    test_bin_path_resolution($target, $binDir, $releaseDir, $exe, true, true),
    "Should use binPath if it exists"
);

assert_equals(
    "$releaseDir/$target/$exe",
    test_bin_path_resolution($target, $binDir, $releaseDir, $exe, false, true),
    "Should fallback to releasePath if binPath is missing but releasePath exists"
);

assert_equals(
    "$binDir/$target/$exe",
    test_bin_path_resolution($target, $binDir, $releaseDir, $exe, false, false),
    "Should keep binPath if both are missing (triggering download later)"
);

if (!empty($errors)) {
    echo PHP_EOL . implode(PHP_EOL, $errors) . PHP_EOL;
    exit(1);
}

echo PHP_EOL . "All wrapper logic tests passed successfully!" . PHP_EOL;
exit(0);
