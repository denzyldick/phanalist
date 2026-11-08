<?php
// Bad: HTTP response splitting via header
$header = $_GET['header'] ?? '';
header($header);
