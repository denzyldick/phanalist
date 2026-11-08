<?php
// Bad: SSRF via user-controlled URL
$url = $_GET['url'] ?? '';
$content = file_get_contents($url);
