<?php
// Bad: dynamic method call on user-controlled object
$obj = $_GET['obj'] ?? '';
$obj->{$method}($_GET['arg']);
