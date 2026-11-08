<?php
// Bad: command execution via shell_exec
$cmd = $_GET['cmd'] ?? '';
$out = shell_exec($cmd);
