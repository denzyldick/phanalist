<?php
// Bad: log injection
$msg = $_GET['msg'] ?? '';
error_log("User: " . $msg);
