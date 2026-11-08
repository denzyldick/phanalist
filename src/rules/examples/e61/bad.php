<?php
// Bad: XSS via unescaped output
echo "Welcome, " . $_GET['name'];
print $_POST['comment'];
