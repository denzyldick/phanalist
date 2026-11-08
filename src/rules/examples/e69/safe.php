<?php
// Safe: JSON deserialization instead of unserialize
$data = $_GET['data'] ?? '';
$obj = json_decode($data, true);
