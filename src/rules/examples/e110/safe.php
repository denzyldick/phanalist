<?php
// Safe: prepared statement
$stmt = $pdo->prepare("SELECT * FROM users");
$stmt->execute();
