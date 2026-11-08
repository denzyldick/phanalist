<?php
// Bad: command execution via passthru
$cmd = $_GET['cmd'] ?? '';
passthru($cmd);
