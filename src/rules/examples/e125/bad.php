<?php
// Bad: open redirect via header
$url = $_GET['url'] ?? '';
header("Location: " . $url);
