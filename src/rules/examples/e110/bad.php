<?php
// Bad: raw SQL query execution
$result = mysqli_query($conn, "SELECT * FROM users");
