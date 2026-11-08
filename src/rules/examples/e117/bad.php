<?php
// Bad: SQL injection via query string
$id = $_GET['id'] ?? '';
mysqli_query($conn, "SELECT * FROM users WHERE id = " . $id);
