<?php
// Safe: prepared statements with bound parameters
$id = $_GET['id'] ?? '';
$stmt = $pdo->prepare("SELECT * FROM users WHERE id = ?");
$stmt->execute([$id]);
$user = $stmt->fetch();
