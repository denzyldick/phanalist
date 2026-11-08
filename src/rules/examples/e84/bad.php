<?php
// Bad: SQL query string concatenation
$id = $_GET['id'] ?? '';
mysqli_query($conn, "SELECT * FROM users WHERE id = " . $id);
