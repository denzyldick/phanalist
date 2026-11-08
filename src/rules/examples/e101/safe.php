<?php
// Safe: JSON deserialization
$data = $_GET['data'] ?? '';
$obj = json_decode($data, true);
