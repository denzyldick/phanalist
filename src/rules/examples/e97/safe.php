<?php
// Safe: cookie with HttpOnly flag
setcookie("session", "abc", 0, "/", "", false, true);
