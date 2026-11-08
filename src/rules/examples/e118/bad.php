<?php
// Bad: log forging via user input
$msg = $_GET['msg'] ?? '';
error_log("User: " . $msg);
