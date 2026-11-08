<?php
// Bad: backtick command with interpolation
$dir = $_GET['dir'] ?? '';
$output = `ls $dir`;
