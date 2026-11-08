<?php
// Bad: reflection injection
$method = $_GET['method'] ?? '';
$reflection->invoke($method, $_GET['arg']);
