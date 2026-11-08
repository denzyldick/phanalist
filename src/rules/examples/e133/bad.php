<?php
// Bad: header injection via setcookie
$name = $_GET['name'] ?? '';
setcookie("name", $name);
