<?php
// XSS example
echo "Welcome, " . $_GET['name'];
print $_POST['comment'];