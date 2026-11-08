<?php
// Bad: command injection via user-controlled input
$file = $_GET['file'] ?? '';
system("cat $file");
$output = shell_exec("ls " . $_GET['dir']);
