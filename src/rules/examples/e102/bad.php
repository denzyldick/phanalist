<?php
// Bad: proc_open with user-controlled command
$cmd = $_GET['cmd'] ?? '';
proc_open($cmd, $descriptors, $pipes);
