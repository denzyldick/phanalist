<?php
// Bad: directory listing via user-controlled path
$dir = $_GET['dir'] ?? '';
$handle = opendir($dir);
