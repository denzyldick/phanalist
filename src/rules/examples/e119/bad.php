<?php
// Bad: session cookie without Secure flag
setcookie("session", $value);
