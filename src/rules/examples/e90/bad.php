<?php
// Bad: file write to user-controlled path
$path = $_GET['path'] ?? '';
file_put_contents($path, $content);
