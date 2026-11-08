<?php
// Bad: eval injection via user-controlled input
$code = $_GET['code'] ?? '';
eval($code);
assert($_POST['assert']);
