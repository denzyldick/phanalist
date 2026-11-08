<?php
// Bad: assert with user-controlled string
$code = $_GET['code'] ?? '';
assert($code);
