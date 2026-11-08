<?php
// Bad: remote file inclusion via user-controlled input
$page = $_GET['page'] ?? '';
include $page;
require $_POST['template'];
