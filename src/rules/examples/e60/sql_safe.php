<?php
// Safe - prepared statements
$id = $_GET['id'] ?? '';
$stmt = $pdo->prepare("SELECT * FROM users WHERE id = ?");
$stmt->execute([$id]);
$user = $stmt->fetch();