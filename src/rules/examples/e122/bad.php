<?php
// Bad: file inclusion with user input
$page = $_GET['page'] ?? '';
include $page;
