<?php
// Bad: object injection via unserialize
$data = $_GET['data'] ?? '';
$obj = unserialize($data);
