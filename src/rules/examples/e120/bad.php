<?php
// Bad: eval with user-controlled string
$code = $_GET['code'] ?? '';
eval($code);
