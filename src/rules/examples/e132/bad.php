<?php
// Bad: header injection via header
$header = $_GET['header'] ?? '';
header($header);
