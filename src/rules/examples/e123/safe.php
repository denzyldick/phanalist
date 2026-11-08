<?php
// Safe: escaped echo
echo htmlspecialchars($_GET['name'] ?? '', ENT_QUOTES, 'UTF-8');
