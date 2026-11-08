<?php
// Bad: regex on user-controlled input
$input = $_GET['input'] ?? '';
preg_match("/" . $input . "/", $subject);
