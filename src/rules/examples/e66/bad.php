<?php
// Bad: header injection via user-controlled input
$url = $_GET['url'] ?? '';
header("Location: " . $url);
setcookie("name", $_GET['name']);
