<?php
// Bad: deserialization of untrusted data
$data = $_GET['data'] ?? '';
$obj = unserialize($data);
