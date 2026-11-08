<?php
// Bad: command execution via popen with user input
$cmd = $_GET['cmd'] ?? '';
$handle = popen($cmd, "r");
