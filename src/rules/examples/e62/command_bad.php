<?php
// Command injection example
$file = $_GET['file'] ?? '';
system("cat $file");
$output = shell_exec("ls " . $_GET['dir']);