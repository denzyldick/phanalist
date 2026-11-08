<?php
// Bad: unsafe deserialization of user-controlled input
$data = $_GET['data'] ?? '';
$obj = unserialize($data);
$obj = wddx_deserialize($_POST['data']);
