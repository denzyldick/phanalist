<?php
// Safe - escaped output
echo "Welcome, " . htmlspecialchars($_GET['name'] ?? '', ENT_QUOTES, 'UTF-8');
print htmlentities($user->bio, ENT_QUOTES, 'UTF-8');