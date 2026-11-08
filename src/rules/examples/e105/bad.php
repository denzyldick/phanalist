<?php
// Bad: local file inclusion via user input
$page = $_GET['page'] ?? '';
include $page;
