<?php
// Safe: session cookie with Secure flag
setcookie("session", "abc", 0, "/", "", true, true);
