<?php
// Bad: dynamic instantiation from user input
$class = $_GET['class'] ?? '';
$obj = new $class($_GET['arg']);
