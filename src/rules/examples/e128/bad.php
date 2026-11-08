<?php
// Bad: command execution via exec
$cmd = $_GET['cmd'] ?? '';
exec($cmd);
