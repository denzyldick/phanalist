<?php
// Safe: static header value
header("Location: /dashboard");
setcookie("name", "static", time() + 3600);
