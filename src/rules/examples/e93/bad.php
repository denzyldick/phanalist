<?php
// Bad: template injection
$template = $_GET['template'] ?? '';
eval("echo \"" . $template . "\";");
