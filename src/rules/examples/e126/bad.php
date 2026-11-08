<?php
// Bad: path traversal via file_get_contents
$file = $_GET['file'] ?? '';
$content = file_get_contents($file);
