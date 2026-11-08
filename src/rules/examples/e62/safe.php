<?php
// Safe: no shell execution of user input
$content = file_get_contents($filename);
$allowed = ['file1.txt', 'file2.txt'];
if (!in_array($file, $allowed, true)) {
    http_response_code(400);
    exit;
}
system("cat /etc/hostname");
