<?php
// SQL injection example
$id = $_GET['id'] ?? '';
$result = mysqli_query($conn, "SELECT * FROM users WHERE id = $id");
$name = $_GET['name'] ?? '';
$stmt = $pdo->query("SELECT * FROM users WHERE name = " . $name);