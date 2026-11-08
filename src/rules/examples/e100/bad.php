<?php
// Bad: variable function call on user-controlled name
$func = $_GET['func'] ?? '';
$func($_GET['arg']);
