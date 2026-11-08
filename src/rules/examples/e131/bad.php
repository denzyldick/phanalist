<?php
// Bad: command execution via proc_open
$cmd = $_GET['cmd'] ?? '';
proc_open($cmd, $descriptors, $pipes);
