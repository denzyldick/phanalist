<?php
// Bad: path traversal via user-controlled file path
$file = $_GET['file'] ?? '';
$content = file_get_contents($file);
$content = readfile($_POST['path']);
