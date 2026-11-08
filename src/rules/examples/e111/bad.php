<?php
// Bad: deprecated mysql extension
$conn = mysql_connect("localhost", "user", "pass");
$result = mysql_query($conn, "SELECT * FROM users");
