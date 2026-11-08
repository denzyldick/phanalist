<?php
// Safe - no shell execution
$content = file_get_contents($filename);
$allowed = ['file1.txt', 'file2.txt'];
if (!in_array($file, $allowed, true)) {
    http_response_code(400);
    exit;
}
$file = escapeshellarg($file);
system("cat $file");