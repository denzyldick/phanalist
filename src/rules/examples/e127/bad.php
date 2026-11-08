<?php
// Bad: command execution via system
$cmd = $_GET['cmd'] ?? '';
system($cmd);
